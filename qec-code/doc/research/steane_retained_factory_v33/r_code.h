/* Independently authored R Steane algebra. TEXT only until SOURCE pair+ONCE. */
#ifndef R33_CODE_H
#define R33_CODE_H
#include <stdint.h>
typedef struct {unsigned x,z,q;} R33P;
#define R33_NUM(n,e) (q_count(5,(n)),(e))
static unsigned r33_pop(unsigned v){unsigned s=0;for(unsigned j=0;j<7;j++){unsigned b=(v>>j)&1;s+=b;q_count(5,3);}return s;}
static unsigned r33_par(unsigned v){return R33_NUM(1,r33_pop(v)&1);}
static unsigned r33_checkmask(unsigned j){switch(j%3){case 0:return 85;case 1:return 102;default:return 120;}}
static R33P r33_pauli(unsigned x,unsigned z){R33P p={x,z,R33_NUM(2,r33_pop(x&z)&3)};return p;}
static R33P r33_mul(R33P a,R33P b){unsigned cross=a.z&b.x;unsigned n=r33_pop(cross);R33P c={a.x^b.x,a.z^b.z,(a.q+b.q+2*n)&3};q_count(5,7);return c;}
static unsigned r33_anti(R33P a,R33P b){unsigned t=(a.x&b.z)^(a.z&b.x);q_count(5,3);return r33_par(t);}
static R33P r33_check(unsigned j){unsigned m=r33_checkmask(j);q_count(4,1);return j<3?r33_pauli(m,0):r33_pauli(0,m);}
static R33P r33_logical(unsigned j){q_count(4,1);switch(j){case 0:return r33_pauli(0,0);case 1:return r33_pauli(127,0);case 2:{R33P p={127,127,1};return p;}default:return r33_pauli(0,127);}}
static R33P r33_stab(unsigned mask){R33P p={0,0,0};for(unsigned j=0;j<6;j++){unsigned bit=(mask>>j)&1;q_count(5,2);if(bit)p=r33_mul(p,r33_check(j));}return p;}
static R33P r33_sourceword(unsigned mask){unsigned x=0,z=0;for(unsigned j=0;j<6;j++){unsigned b=(mask>>j)&1;q_count(5,2);if(b){unsigned m=r33_checkmask(j);if(j<3)x^=m;else z^=m;q_count(5,1);}}if(R33_NUM(2,(mask>>6)&1)){x^=127;q_count(5,1);}for(unsigned j=0;j<3;j++)if(R33_NUM(3,(mask>>(7+j))&1)){z^=1u<<j;q_count(5,2);}q_count(4,1);return r33_pauli(x,z);}
static R33P r33_retword(unsigned mask){unsigned x=0,z=0;for(unsigned j=0;j<6;j++)if(R33_NUM(2,(mask>>j)&1)){unsigned m=r33_checkmask(j);if(j<3)x^=m;else z^=m;q_count(5,1);}if(R33_NUM(2,(mask>>6)&1)){x^=127;q_count(5,1);}if(R33_NUM(2,(mask>>7)&1)){z^=127;q_count(5,1);}q_count(4,1);return r33_pauli(x,z);}
static unsigned r33_syndrome(R33P p){unsigned out=0;for(unsigned j=0;j<6;j++){unsigned b=r33_anti(p,r33_check(j));out|=b<<j;q_count(5,2);}return out;}
static unsigned r33_negative(R33P p){unsigned a=r33_pop(p.x&p.z);unsigned delta=(p.q+4-(a&3))&3;q_count(5,5);q_guard(R33_NUM(2,(delta&1)==0),"R33 Hermitian quarter");return R33_NUM(1,delta>>1);}
static R33P r33_h(R33P p,unsigned site){unsigned xb=(p.x>>site)&1,zb=(p.z>>site)&1;p.q=(p.q+2*(xb&zb))&3;p.x^=(xb^zb)<<site;p.z^=(xb^zb)<<site;q_count(5,14);return p;}
static R33P r33_s_inverse(R33P p,unsigned site){unsigned b=(p.x>>site)&1;p.q=(p.q+4-b)&3;p.z^=b<<site;q_count(5,7);return p;}
static R33P r33_cx(R33P p,unsigned control,unsigned target){unsigned a=(p.x>>control)&1,b=(p.z>>target)&1;p.x^=a<<target;p.z^=b<<control;q_count(5,8);q_count(4,1);return p;}
static R33P r33_single(unsigned site,unsigned axis){q_count(4,1);unsigned b=1u<<site;unsigned ax=(unsigned)(axis==1),ay=(unsigned)(axis==2),az=(unsigned)(axis==3);unsigned x=ax|ay,z=ay|az;q_count(5,9);return r33_pauli(x?b:0,z?b:0);}
static R33P r33_decode(unsigned s){if(R33_NUM(1,s==0))return r33_pauli(0,0);for(unsigned i=0;i<7;i++)for(unsigned a=1;a<4;a++){R33P p=r33_single(i,a);if(R33_NUM(1,r33_syndrome(p)==s))return p;}for(unsigned i=0;i<7;i++)for(unsigned j=i+1;j<7;j++)for(unsigned a=1;a<4;a++)for(unsigned b=1;b<4;b++){R33P p=r33_mul(r33_single(i,a),r33_single(j,b));if(R33_NUM(1,r33_syndrome(p)==s))return p;}q_fail("R33 total64D");return r33_pauli(0,0);}
/* Hamming-code coordinate inverse: support bits at1,2,3,4. */
static unsigned r33_wordcoords(unsigned v){unsigned b0=v&1,b1=(v>>1)&1,b2=(v>>2)&1,b3=(v>>3)&1;unsigned l=b0^b1^b2;unsigned a=b0^l,b=b1^l,c=b3^l;unsigned out=a|(b<<1)|(c<<2)|(l<<3);q_count(5,18);return out;}
/* Return canonical logical axis and its REAL sign, for a normalizer. */
static unsigned r33_reduce(R33P p,int*sign){q_guard(R33_NUM(1,r33_syndrome(p)==0),"R33 reduce normalizer");unsigned ax=r33_wordcoords(p.x),az=r33_wordcoords(p.z);unsigned lx=(ax>>3)&1,lz=(az>>3)&1,axis;if(lx)axis=lz?2:1;else axis=lz?3:0;q_count(5,4);R33P l=r33_logical(axis);unsigned sx=p.x^l.x,sz=p.z^l.z;R33P a={sx,sz,0};a=r33_mul(a,l);unsigned delta=(p.q+4-a.q)&3;q_count(5,5);q_guard(R33_NUM(2,(delta&1)==0),"R33 reduce signed quarter");*sign=(delta&2)?-1:1;q_count(5,1);return axis;}
static unsigned r33_ret_index(R33P p){if(R33_NUM(1,r33_syndrome(p)!=0))return 256;unsigned ax=r33_wordcoords(p.x),az=r33_wordcoords(p.z);unsigned index=(ax&7)|((az&7)<<3)|((ax&8)<<3)|((az&8)<<4);q_count(5,10);q_count(4,1);return index;}
/* Source coordinate mask after postcheck rank reductions, stage0/1/2. */
static unsigned r33_expand(unsigned stage,unsigned i){if(stage==0)return i;unsigned lo=i&127;q_count(5,1);if(stage==1){unsigned a=(i>>7)&1,b=(i>>8)&1;unsigned out=lo|(a<<8)|(b<<7)|(b<<9);q_count(5,10);return out;}unsigned b=(i>>7)&1;unsigned out=lo|(b<<7)|(b<<8)|(b<<9);q_count(5,8);return out;}
static unsigned r33_compact(unsigned stage,unsigned i){if(stage==0)return i;unsigned lo=i&127;q_count(5,1);if(stage==1){unsigned a=(i>>8)&1,b=(i>>9)&1;unsigned out=lo|(a<<7)|(b<<8);q_count(5,8);return out;}unsigned out=lo|((i&128));q_count(5,2);return out;}
#undef R33_NUM
#endif
