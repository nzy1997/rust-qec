#ifndef BACON9_PROTECTED28_V41_C_CODE_H
#define BACON9_PROTECTED28_V41_C_CODE_H
/* Independent code algebra. k is the ABSOLUTE quarter of X^x Z^z. */
typedef struct {uint32_t x,z,k;} C41Pauli;
static void c41_gtest(int b,unsigned n,const char*s){q_count(5,n);q_guard(b,s);}
static unsigned c41_pop(uint32_t a){unsigned n=0;for(;;){int nz=a!=0;q_count(5,1);if(nz){}else break;a&=a-1;n++;q_count(5,3);}return n;}
static C41Pauli c41_p(uint32_t x,uint32_t z,unsigned k){C41Pauli a={x,z,k&3u};q_count(5,1);return a;}
static C41Pauli c41_pos(uint32_t x,uint32_t z){uint32_t w=x&z;q_count(5,1);return c41_p(x,z,c41_pop(w));}
static C41Pauli c41_mul(C41Pauli a,C41Pauli b){unsigned n=c41_pop(a.z&b.x);q_count(5,1);uint32_t x=a.x^b.x,z=a.z^b.z;unsigned k=a.k+b.k+2*n;q_count(5,5);return c41_p(x,z,k);}
static unsigned c41_anti(C41Pauli a,C41Pauli b){unsigned x=c41_pop(a.x&b.z),z=c41_pop(a.z&b.x);q_count(5,2);unsigned n=(x+z)&1u;q_count(5,2);return n;}
static int c41_same(C41Pauli a,C41Pauli b){int n=(a.x==b.x)&(a.z==b.z)&(a.k==b.k);q_count(5,5);return n;}
static unsigned c41_relative(C41Pauli a,C41Pauli b){c41_gtest((a.x==b.x)&(a.z==b.z),3,"C41 equal-support sign");unsigned d=(a.k-b.k)&3u;q_count(5,2);c41_gtest((d==0)|(d==2),3,"C41 Hermitian real sign");q_count(5,1);return d>>1;}
static C41Pauli c41_h(C41Pauli p,unsigned a){uint32_t x=(p.x>>a)&1u,z=(p.z>>a)&1u;q_count(5,4);p.x^=(x^z)<<a;p.z^=(x^z)<<a;p.k=(p.k+2*(x&z))&3u;q_count(5,10);return p;}
static C41Pauli c41_sback(C41Pauli p,unsigned a){uint32_t x=(p.x>>a)&1u;q_count(5,2);p.z^=x<<a;p.k=(p.k+3*x)&3u;q_count(5,5);return p;}
static C41Pauli c41_cx(C41Pauli p,unsigned a,unsigned b){uint32_t x=(p.x>>a)&1u,z=(p.z>>b)&1u;q_count(5,4);p.x^=x<<b;p.z^=z<<a;q_count(5,4);return p;}
static C41Pauli c41_gen(unsigned j){q_count(4,1);if(j<6){unsigned r=j/2,c=j%2,s=3*r+c;uint32_t x=3u<<s;q_count(5,5);return c41_p(x,0,0);}unsigned r=j-6;uint32_t z=63u<<(3*r);q_count(5,3);return c41_p(0,z,0);}
static C41Pauli c41_logic(unsigned j){q_count(4,1);switch(j){case 0:return c41_p(0,0,0);case 1:return c41_p(73,0,0);case 2:return c41_p(73,7,1);default:return c41_p(0,7,0);}}
static C41Pauli c41_extlogic(unsigned j){switch(j){case 1:return c41_logic(3);case 2:{C41Pauli p=c41_logic(2);p.k=(p.k+2)&3u;q_count(5,2);return p;}case 3:return c41_logic(1);default:return c41_logic(0);}}
static C41Pauli c41_stab(unsigned t){C41Pauli p=c41_p(0,0,0);for(unsigned j=0;j<8;j++){unsigned b=(t>>j)&1u;q_count(5,2);if(b)p=c41_mul(p,c41_gen(j));}return p;}
static unsigned c41_synd(C41Pauli p){unsigned s=0;for(unsigned j=0;j<8;j++){unsigned b=c41_anti(p,c41_gen(j));s|=b<<j;q_count(5,2);}return s;}
static unsigned c41_tcol(unsigned t){q_count(4,1);switch(t){case 1:return 0;case 3:return 1;case 2:return 2;default:q_fail("C41 nonzero two-check column");return 0;}}
static C41Pauli c41_decoder(unsigned syn){unsigned u[3];for(unsigned r=0;r<3;r++){u[r]=(syn>>(2*r))&3u;q_count(5,3);}unsigned v=(syn>>6)&3u,chi=u[0]^u[1]^u[2];q_count(5,4);C41Pauli p=c41_p(0,0,0);int present=v!=0;q_count(5,1);if(present){unsigned r=c41_tcol(v);p.x=1u<<(3*r);q_count(5,2);}present=chi!=0;q_count(5,1);if(present){unsigned c=c41_tcol(chi);p.z=1u<<c;q_count(5,1);}unsigned a0=u[0]^chi,a2=u[2];q_count(5,1);/* E_c has Z only on row0; X does not change A syndromes. */
 present=a0!=0;q_count(5,1);if(present){unsigned c=c41_tcol(a0);p.z^=(1u<<c)|(1u<<(c+3));q_count(5,5);}present=a2!=0;q_count(5,1);if(present){unsigned c=c41_tcol(a2);p.z^=(1u<<(c+3))|(1u<<(c+6));q_count(5,6);}return c41_pos(p.x,p.z);}
static uint32_t c41_pack(C41Pauli p){uint32_t w=p.x|(p.z<<9)|(p.k<<18);q_count(5,4);return w;}
static C41Pauli c41_unpack(uint32_t p){uint32_t x=p&511u,z=(p>>9)&511u,k=(p>>18)&3u;q_count(5,5);return c41_p(x,z,k);}
static unsigned c41_rowlabel(C41Pauli p){unsigned x=p.x&7u,z=p.z&7u;q_count(5,2);unsigned n=x|(z<<3);q_count(5,2);return n;}
static C41Pauli c41_rowpauli(unsigned n){q_count(4,1);uint32_t x=n&7u,z=n>>3;q_count(5,2);return c41_pos(x,z);}
static uint32_t c41_outside(C41Pauli p){uint32_t x=p.x>>3,z=p.z>>3,w=x|(z<<6);q_count(5,4);return w;}
static unsigned c41_insert(uint32_t piv[12],uint16_t word[12],uint32_t v,unsigned m){for(int b=11;b>=0;b--){uint32_t hit=(v>>b)&1u;q_count(5,2);if(hit){}else continue;q_count(4,1);int defined=piv[b]!=0;q_count(5,1);if(defined){v^=piv[b];m^=word[b];q_count(5,3);}else{piv[b]=v;word[b]=(uint16_t)m;q_count(5,1);return 1;}}return 0;}
static unsigned c41_solve(const uint32_t piv[12],const uint16_t word[12],uint32_t v,uint32_t*rem){unsigned m=0;for(int b=11;b>=0;b--){uint32_t hit=(v>>b)&1u;q_count(5,2);if(hit){}else continue;q_count(4,2);v^=piv[b];unsigned w=word[b];m^=w;q_count(5,3);}*rem=v;return m;}
#endif
