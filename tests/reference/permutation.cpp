// Calls the unmodified NIST statistics and shuffle functions with a fixed seed.
#include "iid/iid_test_case.h"

#include "iid/permutation_tests.h"
#include <fstream>
#include <iterator>
int main(int argc,char**argv) {
 if(argc<2) return 2;
 std::ifstream file(argv[1],std::ios::binary);
 std::vector<uint8_t> raw((std::istreambuf_iterator<char>(file)),{}), mapped(raw.size());
 if(raw.size()<257) return 3;
 int counts[256]={}, ranks[256]={}, k=0; double mean=0;
 for(auto x:raw) {counts[x]++;mean+=x;} mean/=raw.size();
 for(int i=0;i<256;i++) if(counts[i]) ranks[i]=k++;
 for(size_t i=0;i<raw.size();i++) mapped[i]=ranks[raw[i]];
 auto sorted=mapped;std::sort(sorted.begin(),sorted.end());
 double median=(sorted[(sorted.size()-1)/2]+sorted[sorted.size()/2])/2.0;
 data_t dp={};dp.len=raw.size();dp.alph_size=k;dp.maxsymbol=*std::max_element(raw.begin(),raw.end());
 bool active[19];std::fill(active,active+19,true);long double original[19],values[19];
 run_tests(&dp,mapped.data(),raw.data(),mean,median,original,active);
 uint64_t seed[4]={1,2,3,4};int counters[19][3]={};int rounds=0;
 if(argc>2) for(int r=0;r<10000;r++) {
  FYshuffle(mapped.data(),raw.data(),raw.size(),seed);
  run_tests(&dp,mapped.data(),raw.data(),mean,median,values,active);rounds++;
  bool done=true;
  for(int i=0;i<19;i++) {
   if(active[i]) {counters[i][values[i]>original[i]?0:values[i]==original[i]?1:2]++;
    if(counters[i][0]+counters[i][1]>5 && counters[i][1]+counters[i][2]>5) active[i]=false;
   }
   if(active[i])done=false;
  }
  if(done)break;
 }
 std::cout<<std::setprecision(22);
 for(int i=0;i<19;i++) std::cout<<original[i]<<" "<<counters[i][0]<<" "<<counters[i][1]<<" "<<counters[i][2]<<"\n";
 std::cout<<rounds<<"\n";
}
