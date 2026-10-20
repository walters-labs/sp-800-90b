// Keep the reference algorithms unchanged, but replace OS seeding at the
// boundary so the full CLI workflow is reproducible with one OpenMP thread.
#define seed nist_os_seed
#include "shared/utils.h"
#undef seed
void seed(uint64_t *state) {
 for(int i=0;i<4;i++) state[i]=i+1;
}
#define main nist_restart_cli_main
#include "restart_main.cpp"
#undef main
int main(int argc,char**argv) {
 if(argc>1 && std::string(argv[1])=="--cli") {
  omp_set_num_threads(1);
  return nist_restart_cli_main(argc-1,argv+1);
 }
 if(argc!=3)return 2;
 double h=std::stod(argv[1]),p=pow(2.0,-h);size_t rounds=std::stoul(argv[2]);
 uint64_t state[4]={1,2,3,4};std::vector<uint16_t> results(rounds);
 for(size_t i=0;i<rounds;i++)results[i]=simulateCount(ceil(1.0/p),p,state);
 std::sort(results.begin(),results.end());
 double alpha=1-exp(log(0.99)/2000.0);
 std::cout<<results[(size_t)floor((1-alpha)*rounds)-1]<<"\n";
}
