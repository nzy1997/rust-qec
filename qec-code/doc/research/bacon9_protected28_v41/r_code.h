#ifndef R41_CODE_H
#define R41_CODE_H
/* Independently authored signed Bacon9 algebra. Operative code data are owned
   by the verifier's allocated R41Code; this header has no numeric arrays. */
typedef struct { uint32_t x,z,k; } r41_P;
static unsigned r41_code_pop(uint32_t x){unsigned n=0;int active=x!=0;q_count(5,1);while(active){n+=x&1u;x>>=1;q_count(5,3);active=x!=0;q_count(5,1);}return n;}
static r41_P r41_code_pos(uint32_t x,uint32_t z){uint32_t overlap=x&z;q_count(5,1);unsigned k=r41_code_pop(overlap)&3u;q_count(5,1);return(r41_P){x,z,k};}
static r41_P r41_code_mul(r41_P a,r41_P b){uint32_t cross=a.z&b.x;uint32_t k=(a.k+b.k+2u*r41_code_pop(cross))&3u;uint32_t x=a.x^b.x,z=a.z^b.z;q_count(5,7);return(r41_P){x,z,k};}
static unsigned r41_code_anti(r41_P a,r41_P b){uint32_t cross=(a.x&b.z)^(a.z&b.x);unsigned v=r41_code_pop(cross)&1u;q_count(5,4);return v;}
static uint32_t r41_code_pack(r41_P a){q_count(5,4);return a.x|(a.z<<9)|(a.k<<18);}
static r41_P r41_code_unpack(uint32_t w){r41_P p={w&511u,(w>>9)&511u,(w>>18)&3u};q_count(5,5);return p;}
static r41_P r41_code_gen(unsigned j){q_count(4,1);q_guard(j<8,"R41 generator index");if(j<6){unsigned row=j/2,col=j%2,sh=3u*row+col;uint32_t x=3u<<sh;q_count(5,5);return(r41_P){x,0,0};}unsigned row=j-6,sh=3u*row;uint32_t z=63u<<sh;q_count(5,3);return(r41_P){0,z,0};}
static r41_P r41_code_log(unsigned j){q_count(4,1);switch(j){case 0:return(r41_P){0,0,0};case 1:return(r41_P){0,7,0};case 2:return(r41_P){73,7,3};case 3:return(r41_P){73,0,0};default:q_fail("R41 logical index");return(r41_P){0,0,0};}}
static unsigned r41_code_col(unsigned v){q_count(4,1);switch(v){case 1:return 0;case 3:return 1;case 2:return 2;default:q_fail("R41 nonzero two-character");return 0;}}
static unsigned r41_code_syn(r41_P p){unsigned s=0;for(unsigned j=0;j<8;j++){unsigned b=r41_code_anti(p,r41_code_gen(j));s|=b<<j;q_count(5,2);}return s;}
static r41_P r41_code_D(unsigned s){q_count(4,1);unsigned u0=s&3u,u1=(s>>2)&3u,u2=(s>>4)&3u,v=(s>>6)&3u;unsigned chi=u0^u1^u2;uint32_t x=0,z=0;q_count(5,9);int nonzero=v!=0;q_count(5,1);if(nonzero){unsigned row=r41_code_col(v),site=3u*row;x=1u<<site;q_count(5,2);}nonzero=chi!=0;q_count(5,1);if(nonzero){unsigned col=r41_code_col(chi);z=1u<<col;q_count(5,1);}r41_P ec=r41_code_pos(x,z);unsigned a0=u0,a2=u2;for(unsigned j=0;j<2;j++){a0^=r41_code_anti(ec,r41_code_gen(j))<<j;a2^=r41_code_anti(ec,r41_code_gen(4u+j))<<j;q_count(5,5);}nonzero=a0!=0;q_count(5,1);if(nonzero){unsigned c=r41_code_col(a0);z^=(1u<<c)|(1u<<(3u+c));q_count(5,5);}nonzero=a2!=0;q_count(5,1);if(nonzero){unsigned c=r41_code_col(a2);z^=(1u<<(3u+c))|(1u<<(6u+c));q_count(5,6);}return r41_code_pos(x,z);}
static r41_P r41_code_S(unsigned a){q_count(4,1);r41_P p={0,0,0};for(unsigned j=0;j<8;j++){unsigned selected=(a>>j)&1u;q_count(5,2);if(selected)p=r41_code_mul(p,r41_code_gen(j));}return p;}
/* Outside restriction inverse, with top-row XX as the two free coordinates.
   It is qualified against every signed S word by the caller. */
static unsigned r41_code_outside_rep(r41_P p){q_count(4,1);unsigned m0=(p.x>>3)&1u,m2=(p.x>>5)&1u,b0=(p.x>>6)&1u,b2=(p.x>>8)&1u,zm=(p.z>>3)&1u,zb=(p.z>>6)&1u;unsigned a=(m0<<2)|(m2<<3)|(b0<<4)|(b2<<5)|((zm^zb)<<6)|(zb<<7);q_count(5,24);return a;}
#endif
