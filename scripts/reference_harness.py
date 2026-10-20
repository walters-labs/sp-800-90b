#!/usr/bin/env python3
"""Build a pinned NIST snapshot and run the Rust reference integration tests."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[1]
MANIFEST = json.loads((REPO / "tests/reference/manifest.json").read_text())


def baseline_values(text):
    values = {}
    for line in text.splitlines():
        if "Estimate:" in line or "Assessed" in line:
            match = re.fullmatch(r"([^=]+) = ([-+0-9.eE]+)", line)
            if match:
                values[match[1].strip()] = float(match[2])
    return values


def allowed_baseline_difference(name, actual, expected, mantissa, allow):
    """Narrow allowance; never hide missing/extra fields or nonfinite values."""
    if not allow or mantissa != 53 or not expected or actual.keys() != expected.keys():
        return False
    allowed = MANIFEST["binary64_baseline_exceptions"].get(name, [])
    found_difference = False
    for label, value in actual.items():
        reference = expected[label]
        if not math.isfinite(value) or not math.isfinite(reference):
            return False
        delta = abs(value - reference)
        scale = max(abs(value), abs(reference))
        upstream_delta = min(delta, delta / scale if scale else 0)
        if upstream_delta >= 1e-10:
            found_difference = True
            if label not in allowed or delta > MANIFEST["binary64_baseline_absolute_tolerance"]:
                return False
    return found_difference


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, default=Path(os.environ.get(
        "NIST_REFERENCE_DIR", REPO.parent / "SP800-90B_EntropyAssessment")))
    parser.add_argument("--fetch", action="store_true", help="fetch the pinned commit into the isolated run directory")
    parser.add_argument("--output", type=Path, default=REPO / "target/reference-runs")
    parser.add_argument("--cxx", default=os.environ.get("CXX", "g++"))
    parser.add_argument("--cppflags", default=os.environ.get("CPPFLAGS", ""))
    parser.add_argument("--ldflags", default=os.environ.get("LDFLAGS", ""))
    parser.add_argument("--allow-binary64-baseline-differences", action="store_true")
    parser.add_argument("--require-extended-precision", action="store_true")
    parser.add_argument("--timeout", type=int, default=1800, help="seconds per build/test command")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run = args.output.resolve() / f"{stamp}-{os.getpid()}"
    run.mkdir(parents=True)
    report = {"status": "running", "reference": MANIFEST, "platform": platform.platform(),
              "commands": [], "baseline": [], "run_directory": str(run)}
    project_files = [REPO / "Cargo.toml", REPO / "Cargo.lock"]
    for folder in ["src", "tests", "scripts", ".github"]:
        project_files.extend(p for p in (REPO / folder).rglob("*")
                             if p.is_file() and "__pycache__" not in p.parts)
    report["project_files_sha256"] = {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest()
                                      for p in sorted(project_files)}
    env = dict(os.environ, OMP_NUM_THREADS="1", LC_ALL="C")

    def command(argv, *, cwd=REPO, input_text=None, check=True):
        argv = list(map(str, argv))
        index = len(report["commands"])
        log = run / f"{index:03d}-{Path(argv[0]).name}.log"
        print(f"[{index}] {shlex.join(argv)}", flush=True)
        started = time.monotonic()
        item = {"argv": argv, "cwd": str(cwd), "log": str(log)}
        report["commands"].append(item)
        try:
            with log.open("w") as out:
                with subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
                                      text=True, stdout=out, stderr=subprocess.STDOUT, start_new_session=True) as process:
                    try:
                        process.communicate(input_text, timeout=args.timeout)
                    except (subprocess.TimeoutExpired, KeyboardInterrupt):
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                        raise
                code = process.returncode
            item["exit_code"] = code
        except subprocess.TimeoutExpired:
            item["timed_out"] = True
            raise
        finally:
            item["seconds"] = round(time.monotonic() - started, 3)
            (run / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        if check and code:
            raise RuntimeError(f"command failed ({code}); see {log}")
        return code, log

    try:
        discovered = {p.stem for p in (REPO / "tests").glob("*reference.rs")}
        if discovered != set(MANIFEST["test_targets"]):
            raise RuntimeError("reference test targets differ from manifest; update the manifest")
        checkout = args.reference.resolve()
        if args.fetch:
            checkout = run / "git"
            command(["git", "init", checkout])
            command(["git", "-C", checkout, "fetch", "--depth=1", MANIFEST["repository"], MANIFEST["commit"]])
        command(["git", "-C", checkout, "cat-file", "-e", MANIFEST["commit"] + "^{commit}"])
        snapshot = run / "nist"
        snapshot.mkdir()
        archive = run / "source.tar"
        command(["git", "-C", checkout, "archive", "--format=tar", "--output", archive, MANIFEST["commit"]])
        command(["tar", "-xf", archive, "-C", snapshot])
        report["source_archive_sha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        cxx = shlex.split(args.cxx)
        if not cxx:
            raise ValueError("empty compiler command")
        command(cxx + ["--version"])
        command(["rustc", "-Vv"])
        command(["cargo", "--version"])
        command(["git", "rev-parse", "HEAD"])
        command(["git", "status", "--short"])
        _, macros = command(cxx + ["-dM", "-E", "-x", "c++", "-"], input_text="")
        mantissa = int(re.search(r"^#define __LDBL_MANT_DIG__ (\d+)$", macros.read_text(), re.M)[1])
        report["long_double_mantissa_bits"] = mantissa
        if args.require_extended_precision and mantissa <= 53:
            raise RuntimeError("this run requires extended-precision long double")
        flags = ["-std=c++11", "-fopenmp", "-O2", "-ffloat-store", "-I" + str(snapshot / "cpp")]
        flags += shlex.split(args.cppflags)
        if not args.cppflags:
            _, pc = command(["pkg-config", "--cflags", "jsoncpp", "openssl"])
            flags += shlex.split(pc.read_text())
        links = shlex.split(args.ldflags)
        if not links:
            _, pc = command(["pkg-config", "--libs", "jsoncpp", "openssl"])
            links += shlex.split(pc.read_text())
        links += ["-lbz2", "-lpthread", "-ldivsufsort", "-ldivsufsort64", "-lmpfr", "-lgmp"]
        for name in ["iid", "non_iid", "restart", "conditioning", "transpose"]:
            command(cxx + flags + [snapshot / f"cpp/{name}_main.cpp", "-o", snapshot / f"cpp/ea_{name}"] + links)
        adapters = {
            "NIST_CHI_SQUARE_REFERENCE": "chi_square_independence",
            "NIST_PERMUTATION_REFERENCE": "permutation",
            "NIST_IID_LRS_REFERENCE": "iid_lrs",
            "NIST_RESTART_REFERENCE": "restart",
        }
        for variable, source in adapters.items():
            binary = run / source
            command(cxx + flags + [REPO / f"tests/reference/{source}.cpp", "-o", binary] + links)
            env[variable] = str(binary)
        env["NIST_REFERENCE_DIR"] = str(snapshot)
        report["test_environment"] = {k: v for k, v in env.items() if k.startswith("NIST_") or k == "OMP_NUM_THREADS"}
        datasets = sorted((snapshot / "bin").glob("*.bin"))
        if len(datasets) != 11:
            raise RuntimeError("pinned reference must contain eleven datasets")
        report["dataset_sha256"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in datasets}
        failures = []
        for data in datasets:
            _, output = command([snapshot / "cpp/ea_non_iid", "-vv", data])
            # stderr is retained with stdout; estimator labels remain parseable.
            result = snapshot / "cpp/selftest" / (data.stem + ".res")
            shutil.copyfile(output, result)
            expected = snapshot / "cpp/selftest/refdata" / result.name
            code, log = command(["perl", snapshot / "cpp/selftest/compareresults.pl", result, expected], check=False)
            actual_values = baseline_values(result.read_text())
            expected_values = baseline_values(expected.read_text())
            compatible = code == 1 and allowed_baseline_difference(data.stem, actual_values, expected_values,
                                      mantissa, args.allow_binary64_baseline_differences)
            # NIST's script only prints extra fields; enforce matching fields too.
            fields_match = bool(expected_values) and actual_values.keys() == expected_values.keys()
            passed = code == 0 and fields_match
            report["baseline"].append({"dataset": data.name, "upstream_exit_code": code,
                "status": "passed" if passed else "allowed_binary64_difference" if compatible else "failed",
                "comparison_log": str(log)})
            if not passed and not compatible:
                failures.append("baseline: " + data.name)
        report["comparisons"] = []
        for target in MANIFEST["test_targets"]:
            code, log = command(["cargo", "test", "--locked", "--test", target, "--", "--ignored", "--nocapture", "--test-threads=1"], check=False)
            report["comparisons"].append({"target": target, "exit_code": code, "log": str(log),
                "test_summaries": re.findall(r"^test result:.*$", log.read_text(), re.M)})
            if code:
                failures.append("Rust comparisons: " + target)
        report["failures"] = failures
        report["status"] = "failed" if failures else ("passed_with_binary64_baseline_differences" if any(
            b["status"] == "allowed_binary64_difference" for b in report["baseline"]) else "passed")
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        report["status"] = "failed"
        report["error"] = str(error)
    finally:
        (run / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"{report['status']}: {run / 'report.json'}", flush=True)
    return 1 if report["status"] == "failed" else 0


if __name__ == "__main__":
    sys.exit(main())
