#ifndef SHOR25_C43_QEC_CODE_EXPORT_H
#define SHOR25_C43_QEC_CODE_EXPORT_H
/* Independent C export of public ShorLikeSpec(5,5), Hphys25 Vstd Hlogical.
 * Positive Hermitian representative is i^pop(x&z) X^x Z^z. No local verifier
 * GF2 engine. Total decoder is an algorithm, not a 2^24 table. SOURCE ONLY. */
typedef struct { uint32_t x,z,k; } C43P;
static unsigned c43_pop(uint32_t x){unsigned n=0;for(;;){int nz=x!=0;q_count(5,1);if(!nz)break;x&=x-1;n++;q_count(5,3);}return n;}
static C43P c43_p(uint32_t x,uint32_t z,unsigned k){C43P p={x,z,k&3u};q_count(5,1);return p;}
static C43P c43_pos(uint32_t x,uint32_t z){q_count(5,1);return c43_p(x,z,c43_pop(x&z));}
static C43P c43_mul(C43P a,C43P b){unsigned k=a.k+b.k+2*c43_pop(a.z&b.x);q_count(5,6);return c43_p(a.x^b.x,a.z^b.z,k);}
static unsigned c43_anti(C43P a,C43P b){unsigned k=(c43_pop(a.x&b.z)+c43_pop(a.z&b.x))&1u;q_count(5,4);return k;}
static unsigned c43_rel(C43P p){unsigned k=(p.k-c43_pop(p.x&p.z))&3u;q_count(5,3);return k;}
static C43P c43_h(C43P p,unsigned a){unsigned x=(p.x>>a)&1u,z=(p.z>>a)&1u,bit=1u<<a;q_count(5,5);p.k=(p.k+2*x*z)&3u;p.x^=(x^z)*bit;p.z^=(x^z)*bit;q_count(5,10);return p;}
static C43P c43_cx(C43P p,unsigned a,unsigned b){p.x^=((p.x>>a)&1u)<<b;p.z^=((p.z>>b)&1u)<<a;q_count(5,8);return p;}
static C43P c43_sb(C43P p,unsigned a){unsigned x=(p.x>>a)&1u;q_count(5,2);p.k=(p.k+3*x)&3u;p.z^=x<<a;q_count(5,5);return p;}
static C43P c43_gen(unsigned i){q_count(4,1);if(i<20){unsigned r=i/4,j=i%4;q_count(5,5);return c43_p(3u<<(5*r+j),0,0);}unsigned r=i-20;q_count(5,7);return c43_p(0,31u<<(5*r)|31u<<(5*r+5),0);}
static C43P c43_logical(unsigned a){q_count(4,1);if(a==0)return c43_p(0,0,0);if(a==1)return c43_p(0,31,0);if(a==3)return c43_p(0x108421u,0,0);return c43_p(0x108421u,31,3); /* external Y=-internalY */}
static uint32_t c43_synd(C43P p){uint32_t s=0;for(unsigned i=0;i<24;i++){s|=c43_anti(p,c43_gen(i))<<i;q_count(5,2);}return s;}
static unsigned c43_minchain(unsigned u){unsigned m=0,v=0;for(unsigned j=0;j<4;j++){v^=(u>>j)&1u;m|=v<<(j+1);q_count(5,6);}int high=c43_pop(m)>2;q_count(5,1);if(high){m^=31u;q_count(5,1);}return m;}
static C43P c43_decoder(uint32_t s){uint32_t z=0,x=0;for(unsigned r=0;r<5;r++){z|=c43_minchain((s>>(4*r))&15u)<<(5*r);q_count(5,6);}unsigned y=c43_minchain((s>>20)&15u);q_count(5,2);for(unsigned r=0;r<5;r++){x|=((y>>r)&1u)<<(5*r);q_count(5,5);}return c43_pos(x,z);}
static unsigned c43_reportmask(unsigned r){unsigned a=r&1u,b=(r>>1)&1u,c=(r>>2)&1u,d=(r>>3)&1u;q_count(5,7);unsigned m=(a<<1)|(c<<2)|(b<<3)|((b^d)<<4);q_count(5,8);int high=c43_pop(m)>2;q_count(5,1);if(high){m^=31u;q_count(5,1);}return m;}
static unsigned c43_local(unsigned x,unsigned z){unsigned a=x?(z?2u:1u):(z?3u:0u);q_count(5,2);return a;}
static C43P c43_localp(unsigned label){unsigned a=label&3u,b=label>>2,x=((a==1)|(a==2))|(((b==1)|(b==2))<<1),z=((a==2)|(a==3))|(((b==2)|(b==3))<<1);q_count(5,16);return c43_pos(x,z);}
static unsigned c43_locallabel(C43P p){q_count(5,10);return c43_local(p.x&1u,p.z&1u)+4*c43_local((p.x>>1)&1u,(p.z>>1)&1u);}
static unsigned c43_index(C43P p){q_count(5,2);return p.x|(p.z<<5);}
static C43P c43_rowp(unsigned n){q_count(5,2);return c43_pos(n&31u,n>>5);}
/* Rank24 follows 20 independent row-chain X characters plus 4 independent
 * row-chain Z characters; disjoint X/Z columns and 5 odd. Isometry is the
 * positive even/odd row-parity tensor basis, external domain Hadamard. */
static uint64_t c43_code_qualify(void){uint64_t bad=0;
 for(unsigned i=0;i<24;i++){C43P a=c43_gen(i);c43_wit(&bad,c43_rel(a)==0,1,"C43 positive stabilizer");for(unsigned j=0;j<24;j++)c43_wit(&bad,!c43_anti(a,c43_gen(j)),1,"C43 all24 commuting generators");for(unsigned l=1;l<4;l++)c43_wit(&bad,!c43_anti(a,c43_logical(l)),1,"C43 logical stabilizer commutation");}
 for(unsigned a=1;a<4;a++)for(unsigned b=1;b<4;b++)c43_wit(&bad,c43_anti(c43_logical(a),c43_logical(b))==(a!=b),2,"C43 named logical algebra");
 C43P xy=c43_mul(c43_logical(1),c43_logical(2)),lz=c43_logical(3);c43_wit(&bad,xy.x==lz.x&&xy.z==lz.z&&xy.k==((lz.k+1)&3u),7,"C43 external XY=iZ signed Hlogical frame");
 for(unsigned bit=0;bit<32;bit++){unsigned parity=c43_pop(bit)&1u;for(unsigned j=0;j<4;j++)c43_wit(&bad,(c43_pop(bit^(3u<<j))&1u)==parity,4,"C43 positive row isometry XX");c43_wit(&bad,(c43_pop(bit^1u)&1u)==(parity^1u),4,"C43 row logicalX exact positive amplitudes");}
 for(unsigned n=0;n<1024;n++){C43P p=c43_rowp(n),D=c43_decoder(c43_synd(p)),e=c43_mul(D,p);c43_wit(&bad,c43_synd(e)==0,1,"C43 every active1024 residual in normalizer");c43_wit(&bad,(e.x&~31u)==0&&(e.z==0||e.z==31u)&&!(c43_pop(e.x)&1u),9,"C43 row residual evenX plus I/LZ");}
 for(unsigned u=0;u<16;u++){unsigned z=c43_minchain(u),actual=0;for(unsigned j=0;j<4;j++){actual|=(((z>>j)^(z>>(j+1)))&1u)<<j;q_count(5,6);}c43_wit(&bad,actual==u&&c43_pop(z)<=2,3,"C43 total decoder row boundary basis");}
 for(unsigned n=0;n<32;n++){unsigned k=c43_pop(n);int sum1=5-2*(int)k,sum3=0,prod=(k&1u)?-1:1;for(unsigned s=0;s<32;s++)if(c43_pop(s)==3){sum3+=(c43_pop(s&n)&1u)?-1:1;q_count(3,1);}int f=3*sum1-sum3+3*prod;q_count(3,3);c43_wit(&bad,f==((k<=2)?8:-8),2,"C43 full32 F5 majority exact");}
 return bad;
}
#endif
