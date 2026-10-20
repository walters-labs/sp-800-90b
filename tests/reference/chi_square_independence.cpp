// Project test adapter for NIST SP800-90B_EntropyAssessment (2026-10-20).
// External NIST code retains its notices; see LICENSE-NIST and docs/provenance.md.
// Thin test adapter: uses the external NIST header without copying its implementation.
// Compile with -I<reference>/cpp and the reference's dependency/linker flags.
#include "iid/chi_square_tests.h"
#include <fstream>
#include <iterator>
int main(int argc, char **argv) {
    if (argc != 2 && argc != 3) return 2;
    std::ifstream file(argv[1], std::ios::binary);
    if (!file) return 3;
    std::vector<uint8_t> data((std::istreambuf_iterator<char>(file)), {});
    if (data.size() < 2 || data.size() > INT32_MAX) return 4;
    bool seen[256] = {};
    int ranks[256] = {}, k = 0;
    for (auto symbol : data) seen[symbol] = true;
    for (int i = 0; i < 256; ++i) if (seen[i]) ranks[i] = k++;
    if (k < 2) return 5;
    for (auto &symbol : data) symbol = ranks[symbol];
    double score = 0;
    int df = 0;
    if (argc == 3) {
        if (k == 2) binary_goodness_of_fit(data.data(), score, df, data.size());
        else goodness_of_fit(data.data(), score, df, data.size(), k);
    } else if (k == 2) binary_chi_square_independence(data.data(), score, df, data.size());
    else chi_square_independence(data.data(), score, df, data.size(), k);
    std::cout << std::setprecision(17) << score << " " << df << " " << chi_square_pvalue(score, df) << "\n";
}
