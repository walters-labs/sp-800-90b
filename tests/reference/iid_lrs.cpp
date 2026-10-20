// Project test adapter for NIST SP800-90B_EntropyAssessment (2026-10-20).
// External NIST code retains its notices; see LICENSE-NIST and docs/provenance.md.
// Independent adapter calling the unmodified NIST IID LRS hypothesis test.
#include "shared/lrs_test.h"
#include <fstream>
#include <iterator>
int main(int argc,char**argv) {
 if(argc!=2) return 2;
 std::ifstream file(argv[1],std::ios::binary);
 std::vector<uint8_t> data((std::istreambuf_iterator<char>(file)),{});
 if(data.size()<2 || data.size()>INT32_MAX) return 3;
 bool seen[256]={};int ranks[256]={},k=0;
 for(auto x:data)seen[x]=true;
 for(int i=0;i<256;i++)if(seen[i])ranks[i]=k++;
 for(auto &x:data)x=ranks[x];
 bool pass=len_LRS_test(data.data(),data.size(),k,3,"Literal");
 std::cout<<"pass = "<<pass<<"\n";
}
