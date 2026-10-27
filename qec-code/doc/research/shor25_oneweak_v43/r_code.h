/* R independent algebra export; SOURCE ONLY. ShorLike(5,5), Hphysical25
 * transport, internal W=Hphys Vstd Hlogical; external V=W Hlogical.
 * No verifier-local construction or dense syndrome table. */
#ifndef SHOR25_ONEWEAK_R_CODE_V43_H
#define SHOR25_ONEWEAK_R_CODE_V43_H
#include <stdint.h>
typedef struct {uint32_t x,z,k;} r43_P;
static unsigned r43_pop(uint32_t a){unsigned n=0;for(unsigned j=0;j<25;j++){n+=(a>>j)&1u;q_count(5,3);}return n;}
static unsigned r43_par(uint32_t a){/* finite XOR fold, not a population table */
 a^=a>>16;a^=a>>8;a^=a>>4;a^=a>>2;a^=a>>1;q_count(5,10);q_count(5,1);return a&1u;}
static r43_P r43_pos(uint32_t x,uint32_t z){unsigned k=r43_pop(x&z)&3u;q_count(5,2);return(r43_P){x,z,k};}
static r43_P r43_pmul(r43_P a,r43_P b){unsigned q=(a.k+b.k+2u*r43_par(a.z&b.x))&3u;uint32_t x=a.x^b.x,z=a.z^b.z;q_count(5,8);return(r43_P){x,z,q};}
static unsigned r43_anti(r43_P a,r43_P b){uint32_t t=(a.x&b.z)^(a.z&b.x);q_count(5,3);return r43_par(t);}
static r43_P r43_gen(unsigned j){q_count(4,1);q_guard(j<24,"R43 generator bound");if(j<20){unsigned r=j/4,c=j%4,site=5*r+c;uint32_t x=3u<<site;q_count(5,5);return(r43_P){x,0,0};}unsigned r=j-20;uint32_t z=1023u<<(5*r);q_count(5,3);return(r43_P){0,z,0};}
static r43_P r43_internal(unsigned a){switch(a){case 0:return(r43_P){0,0,0};case 1:return(r43_P){1082401u,0,0};case 2:return(r43_P){1082401u,31u,1};default:return(r43_P){0,31u,0};}}
static r43_P r43_external(unsigned a){switch(a){case 0:return r43_internal(0);case 1:return r43_internal(3);case 2:{r43_P y=r43_internal(2);y.k=3;return y;}default:return r43_internal(1);}}
static uint32_t r43_syn(r43_P p){uint32_t s=0;for(unsigned j=0;j<24;j++){s|=r43_anti(p,r43_gen(j))<<j;q_count(5,2);}return s;}
/* Adjacent-edge path bits -> minimum-weight vertex mask. n=5 odd: no ties. */
static unsigned r43_pathmask(unsigned s){unsigned a=0,v=0;for(unsigned j=0;j<4;j++){v^=(s>>j)&1u;a|=v<<(j+1);q_count(5,6);}unsigned n=r43_pop(a);if(n>2){a^=31u;q_count(5,1);}q_count(5,1);return a;}
static r43_P r43_D(uint32_t syndrome){uint32_t z=0,x=0;for(unsigned r=0;r<5;r++){unsigned v=(syndrome>>(4*r))&15u;z|=r43_pathmask(v)<<(5*r);q_count(5,5);}unsigned rows=r43_pathmask(syndrome>>20);q_count(5,1);for(unsigned r=0;r<5;r++){x|=((rows>>r)&1u)<<(5*r);q_count(5,4);}return r43_pos(x,z);}
static unsigned r43_tmask(unsigned r){unsigned z=((r&1u)<<1)|(((r>>2)&1u)<<2)|(((r>>1)&1u)<<3)|((((r>>1)^(r>>3))&1u)<<4);q_count(5,17);if(r43_pop(z)>2){z^=31u;q_count(5,2);}else q_count(5,1);return z;}
/* Base4 positive onsite I,X,Y,Z. Wire0 is the fastest coordinate. */
static unsigned r43_rowphase(uint32_t x,uint32_t z){unsigned k=0;for(unsigned q=0;q<5;q++){k+=((x&z)>>q)&1u;q_count(5,4);}q_count(5,1);return k&3u;}
static r43_P r43_rowp(unsigned label,unsigned n){q_count(4,1);unsigned x=0,z=0,k=0;for(unsigned q=0;q<n;q++){unsigned v=(label>>(2*q))&3u;x|=((v==1)|(v==2))<<q;z|=((v==2)|(v==3))<<q;k+=(v==2);q_count(5,15);}q_count(5,1);return(r43_P){x,z,k&3u};}
static unsigned r43_label(uint32_t x,uint32_t z,unsigned n){q_count(4,1);unsigned out=0;for(unsigned q=0;q<n;q++){unsigned a=(x>>q)&1u,b=(z>>q)&1u,v=a?(b?2u:1u):(b?3u:0u);out|=v<<(2*q);q_count(5,8);}return out;}
/* Positive Hermitian conjugation under CX(c,t), derived via X-before-Z
 * product, not a shared signed transfer table. */
static r43_P r43_conjCX(r43_P p,unsigned c,unsigned t){unsigned xc=(p.x>>c)&1u,zt=(p.z>>t)&1u;p.x^=xc<<t;p.z^=zt<<c;q_count(5,10);return p;}
#endif
