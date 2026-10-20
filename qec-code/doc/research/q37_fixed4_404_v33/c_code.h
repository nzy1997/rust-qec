#ifndef Q37_FIXED4_C34_CODE_H
#define Q37_FIXED4_C34_CODE_H
/* Independent C algebra, literal Table4. SOURCE TEXT only. No scientific
 * invocation while authoring. No static numeric arrays: all stored coordinates
 * belong to the current provider's actual registered geometry owner. */
static unsigned c34_pop(uint64_t a){unsigned n=0;for(;;){int nz=a!=0;q_count(5,1);if(!nz)break;a&=a-1;n++;q_count(5,3);}return n;}
static unsigned c34_par(uint64_t a){unsigned n=c34_pop(a);q_count(5,1);return n&1u;}
static uint64_t c34_row(unsigned z,unsigned i){q_count(4,1);
#define C34B(j) (UINT64_C(1)<<((j)-1))
 if(!z){switch(i){
 case 0:return C34B(19)|C34B(18);
 case 1:return C34B(33)|C34B(11);
 case 2:return C34B(34)|C34B(12);
 case 3:return C34B(13)|C34B(35);
 case 4:return C34B(37)|C34B(15);
 case 5:return C34B(3)|C34B(21)|C34B(18)|C34B(2);
 case 6:return C34B(22)|C34B(4)|C34B(1)|C34B(16);
 case 7:return C34B(17)|C34B(26)|C34B(6)|C34B(1);
 case 8:return C34B(27)|C34B(26)|C34B(25)|C34B(5);
 case 9:return C34B(7)|C34B(28)|C34B(5)|C34B(24);
 case 10:return C34B(9)|C34B(31)|C34B(32)|C34B(10);
 case 11:return C34B(30)|C34B(8)|C34B(14)|C34B(36);
 case 12:return C34B(7)|C34B(23)|C34B(21)|C34B(20)|C34B(22)|C34B(29);
 case 13:return C34B(9)|C34B(31)|C34B(25)|C34B(24)|C34B(36)|C34B(14);
 case 14:return C34B(34)|C34B(15)|C34B(5)|C34B(6)|C34B(8)|C34B(33)|C34B(19)|C34B(17);
 case 15:return C34B(3)|C34B(22)|C34B(12)|C34B(10)|C34B(2)|C34B(24)|C34B(33)|C34B(35);
 case 16:return C34B(32)|C34B(7)|C34B(27)|C34B(18)|C34B(14)|C34B(15)|C34B(21)|C34B(11);
 case 17:return C34B(13)|C34B(9)|C34B(19)|C34B(26)|C34B(12)|C34B(21)|C34B(30)|C34B(28);
 }}else{switch(i){
 case 0:return C34B(3)|C34B(2);
 case 1:return C34B(16)|C34B(4);
 case 2:return C34B(6)|C34B(17);
 case 3:return C34B(23)|C34B(20);
 case 4:return C34B(20)|C34B(29);
 case 5:return C34B(9)|C34B(28)|C34B(24)|C34B(10);
 case 6:return C34B(26)|C34B(5)|C34B(6)|C34B(28);
 case 7:return C34B(9)|C34B(32)|C34B(30)|C34B(14);
 case 8:return C34B(27)|C34B(25)|C34B(31)|C34B(32);
 case 9:return C34B(7)|C34B(25)|C34B(14)|C34B(23)|C34B(5)|C34B(8);
 case 10:return C34B(27)|C34B(19)|C34B(5)|C34B(21)|C34B(7)|C34B(18);
 case 11:return C34B(7)|C34B(16)|C34B(25)|C34B(24)|C34B(27)|C34B(22);
 case 12:return C34B(27)|C34B(22)|C34B(26)|C34B(21)|C34B(1)|C34B(2);
 case 13:return C34B(31)|C34B(11)|C34B(14)|C34B(8)|C34B(33)|C34B(10);
 case 14:return C34B(26)|C34B(30)|C34B(1)|C34B(25)|C34B(36)|C34B(16);
 case 15:return C34B(34)|C34B(9)|C34B(10)|C34B(12)|C34B(8)|C34B(36);
 case 16:return C34B(10)|C34B(13)|C34B(31)|C34B(36)|C34B(35)|C34B(30);
 case 17:return C34B(32)|C34B(37)|C34B(31)|C34B(8)|C34B(15)|C34B(36);
 }}q_guard(0,"C34 row index");return 0;
}
static uint64_t c34_logical(unsigned z){q_count(4,1);if(z)return C34B(1)|C34B(8)|C34B(9)|C34B(16)|C34B(17)|C34B(30)|C34B(31);return C34B(2)|C34B(3)|C34B(5)|C34B(6)|C34B(7)|C34B(17)|C34B(22);}
#undef C34B

static uint32_t c34_key(unsigned axis,uint64_t e){uint32_t k=0;for(unsigned j=0;j<18;j++){uint64_t row=c34_row(axis^1u,j);q_count(5,1);unsigned p=c34_par(e&row);q_count(5,1);k|=p<<j;q_count(5,2);}uint64_t l=c34_logical(axis^1u);q_count(5,1);unsigned p=c34_par(e&l);q_count(5,1);k|=p<<18;q_count(5,2);return k;}
static uint64_t c34_syndrome(uint64_t x,uint64_t z){uint32_t sx=c34_key(1,z)&262143u,sz=c34_key(0,x)&262143u;q_count(5,2);uint64_t a=sx,b=sz;q_count(5,2);uint64_t out=a|(b<<18);q_count(5,2);return out;}
static unsigned c34_part_size(unsigned i){q_count(4,1);return i<7?3:2;}
static unsigned c34_part_site(unsigned i,unsigned j){q_count(4,1);if(!j)return i;if(i<7){unsigned p=14+2*i+j;q_count(5,3);return p;}unsigned p=i+22;q_count(5,1);return p;}
static unsigned c34_rank(uint64_t *pivot,const uint64_t *v,unsigned n,unsigned width){for(unsigned j=0;j<37;j++)pivot[j]=0;unsigned rank=0;for(unsigned j=0;j<n;j++){uint64_t a=v[j];for(int b=(int)width-1;b>=0;b--){unsigned shift=(unsigned)b;q_count(5,1);uint64_t bit=(a>>shift)&1u;q_count(5,2);if(!bit)continue;q_count(4,1);uint64_t p=pivot[b];int occupied=p!=0;q_count(5,1);if(occupied){a^=p;q_count(5,1);}else{pivot[b]=a;rank++;q_count(5,1);break;}}}return rank;}
#endif
