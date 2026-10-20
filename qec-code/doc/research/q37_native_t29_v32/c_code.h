#ifndef Q37_NATIVE_T29_C_CODE_V32
#define Q37_NATIVE_T29_C_CODE_V32
/* Independent C algebra export. Literal Table 4; no runtime source reader.
 * This header requires q_count/q_guard and stdint types from its includer. */
typedef struct { uint64_t x,z; uint32_t q,pad; } C32Pauli;
static unsigned c32_pop(uint64_t a){unsigned n=0;while(a){a&=a-1;n++;q_count(5,3);}return n;}
static unsigned c32_par(uint64_t a){unsigned n=c32_pop(a);q_count(5,1);return n&1u;}
static uint64_t c32_row(unsigned z,unsigned i){q_count(4,1);
#define C32B(j) (UINT64_C(1)<<((j)-1))
 if(!z){switch(i){
 case 0:return C32B(19)|C32B(18);
 case 1:return C32B(33)|C32B(11);
 case 2:return C32B(34)|C32B(12);
 case 3:return C32B(13)|C32B(35);
 case 4:return C32B(37)|C32B(15);
 case 5:return C32B(3)|C32B(21)|C32B(18)|C32B(2);
 case 6:return C32B(22)|C32B(4)|C32B(1)|C32B(16);
 case 7:return C32B(17)|C32B(26)|C32B(6)|C32B(1);
 case 8:return C32B(27)|C32B(26)|C32B(25)|C32B(5);
 case 9:return C32B(7)|C32B(28)|C32B(5)|C32B(24);
 case 10:return C32B(9)|C32B(31)|C32B(32)|C32B(10);
 case 11:return C32B(30)|C32B(8)|C32B(14)|C32B(36);
 case 12:return C32B(7)|C32B(23)|C32B(21)|C32B(20)|C32B(22)|C32B(29);
 case 13:return C32B(9)|C32B(31)|C32B(25)|C32B(24)|C32B(36)|C32B(14);
 case 14:return C32B(34)|C32B(15)|C32B(5)|C32B(6)|C32B(8)|C32B(33)|C32B(19)|C32B(17);
 case 15:return C32B(3)|C32B(22)|C32B(12)|C32B(10)|C32B(2)|C32B(24)|C32B(33)|C32B(35);
 case 16:return C32B(32)|C32B(7)|C32B(27)|C32B(18)|C32B(14)|C32B(15)|C32B(21)|C32B(11);
 case 17:return C32B(13)|C32B(9)|C32B(19)|C32B(26)|C32B(12)|C32B(21)|C32B(30)|C32B(28);
 }}else{switch(i){
 case 0:return C32B(3)|C32B(2);
 case 1:return C32B(16)|C32B(4);
 case 2:return C32B(6)|C32B(17);
 case 3:return C32B(23)|C32B(20);
 case 4:return C32B(20)|C32B(29);
 case 5:return C32B(9)|C32B(28)|C32B(24)|C32B(10);
 case 6:return C32B(26)|C32B(5)|C32B(6)|C32B(28);
 case 7:return C32B(9)|C32B(32)|C32B(30)|C32B(14);
 case 8:return C32B(27)|C32B(25)|C32B(31)|C32B(32);
 case 9:return C32B(7)|C32B(25)|C32B(14)|C32B(23)|C32B(5)|C32B(8);
 case 10:return C32B(27)|C32B(19)|C32B(5)|C32B(21)|C32B(7)|C32B(18);
 case 11:return C32B(7)|C32B(16)|C32B(25)|C32B(24)|C32B(27)|C32B(22);
 case 12:return C32B(27)|C32B(22)|C32B(26)|C32B(21)|C32B(1)|C32B(2);
 case 13:return C32B(31)|C32B(11)|C32B(14)|C32B(8)|C32B(33)|C32B(10);
 case 14:return C32B(26)|C32B(30)|C32B(1)|C32B(25)|C32B(36)|C32B(16);
 case 15:return C32B(34)|C32B(9)|C32B(10)|C32B(12)|C32B(8)|C32B(36);
 case 16:return C32B(10)|C32B(13)|C32B(31)|C32B(36)|C32B(35)|C32B(30);
 case 17:return C32B(32)|C32B(37)|C32B(31)|C32B(8)|C32B(15)|C32B(36);
 }}q_guard(0,"C32 row index");return 0;
}
static uint64_t c32_logical(unsigned z){q_count(4,1);if(z)return C32B(1)|C32B(8)|C32B(9)|C32B(16)|C32B(17)|C32B(30)|C32B(31);return C32B(2)|C32B(3)|C32B(5)|C32B(6)|C32B(7)|C32B(17)|C32B(22);}
#undef C32B
static C32Pauli c32_mul(C32Pauli a,C32Pauli b){
 C32Pauli r;uint64_t t=a.x&a.z,u=b.x&b.z,v=a.z&b.x;q_count(5,3);
 unsigned p=c32_pop(t),s=c32_pop(u),k=c32_pop(v);r.x=a.x^b.x;r.z=a.z^b.z;t=r.x&r.z;q_count(5,3);
 unsigned w=c32_pop(t);r.q=(a.q+b.q+p+s+2u*k+4u-w)&3u;r.pad=0;q_count(5,8);return r;
}
static C32Pauli c32_cx(C32Pauli a,unsigned c,unsigned t){
 uint64_t v=a.x&a.z;q_count(5,1);unsigned old=c32_pop(v);
 uint64_t xb=(a.x>>c)&1u,zb=(a.z>>t)&1u;a.x^=xb<<t;a.z^=zb<<c;q_count(5,8);
 v=a.x&a.z;q_count(5,1);unsigned now=c32_pop(v);a.q=(a.q+old+4u-now)&3u;q_count(5,4);return a;
}
/* Operand label x+4z, embedded in a three-site part. */
static C32Pauli c32_label(unsigned label,unsigned a,unsigned b){
 C32Pauli p={0,0,0,0};unsigned x=label&3u,z=label>>2;q_count(5,2);
 p.x=((uint64_t)(x&1u)<<a)|((uint64_t)(x>>1)<<b);
 p.z=((uint64_t)(z&1u)<<a)|((uint64_t)(z>>1)<<b);q_count(5,14);return p;
}
static int c32_before(C32Pauli a,C32Pauli b){q_count(5,1);if(a.x<b.x)return 1;q_count(5,1);if(a.x>b.x)return 0;q_count(5,1);return a.z<b.z;}
/* E2 C E1 R E0 R† C, R=exp(-i*pi*Z1Z2/8). */
static unsigned c32_relative(unsigned l0,unsigned l1,unsigned l2,C32Pauli out[2]){
 C32Pauli a=c32_label(l0,0,1),b=c32_label(l1,1,2),e=c32_label(l2,0,1),axis={0,6,0,0};
 uint64_t v=a.x&6u;q_count(5,1);unsigned odd=c32_par(v),n=1;
 out[0]=a;int split=(odd!=0);q_count(5,1);if(split){out[1]=c32_mul(axis,a);out[1].q=(out[1].q+3u)&3u;q_count(5,2);n=2;}
 b=c32_cx(b,0,1);b=c32_mul(e,b);
 for(unsigned j=0;j<n;j++){out[j]=c32_cx(out[j],0,1);out[j]=c32_mul(b,out[j]);}
 if(n==2){int less=c32_before(out[1],out[0]);if(less){C32Pauli tmp=out[0];out[0]=out[1];out[1]=tmp;}}
 unsigned q=out[0].q;for(unsigned j=0;j<n;j++){out[j].q=(out[j].q+4u-q)&3u;q_count(5,3);}return n;
}
#endif
