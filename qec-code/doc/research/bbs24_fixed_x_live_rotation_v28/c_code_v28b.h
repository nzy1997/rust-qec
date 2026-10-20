/* Independent C algebra/declarations. SOURCE TEXT: do not execute before gates. */
#ifndef BBS24_C_CODE_V28_H
#define BBS24_C_CODE_V28_H
typedef struct { uint32_t x,z,k; } CbPauli;
static unsigned cb_pop(uint32_t a){q_count(5,2);return (unsigned)__builtin_popcount(a);}
static uint32_t cb_bit(unsigned a){q_guard(a<25,"C code wire bound");q_count(5,1);return UINT32_C(1)<<a;}
static CbPauli cb_pos(uint32_t x,uint32_t z){CbPauli p={x,z,cb_pop(x&z)&3u};q_count(5,2);return p;}
static unsigned cb_anti(CbPauli a,CbPauli b){unsigned v=cb_pop((a.x&b.z)^(a.z&b.x))&1u;q_count(5,4);return v;}
static CbPauli cb_mul(CbPauli a,CbPauli b){unsigned p=cb_pop(a.z&b.x)&1u;q_count(5,2);CbPauli c={a.x^b.x,a.z^b.z,(a.k+b.k+(p<<1))&3u};q_count(5,6);return c;}
static CbPauli cb_inverse(CbPauli a){unsigned p=cb_pop(a.x&a.z)&1u;q_count(5,2);a.k=((p<<1)+4u-a.k)&3u;q_count(5,4);return a;}
static uint64_t cb_pack(CbPauli p){q_guard(((p.x|p.z)&UINT32_C(0xff000000))==0,"C code basis pack is DATA-only");q_count(5,2);uint64_t r=(uint64_t)p.x|((uint64_t)p.z<<24)|((uint64_t)p.k<<48);q_count(5,7);return r;}
static CbPauli cb_unpack(uint64_t v){CbPauli p={(uint32_t)(v&UINT64_C(0xffffff)),(uint32_t)((v>>24)&UINT64_C(0xffffff)),(uint32_t)((v>>48)&3u)};q_count(5,8);return p;}
static CbPauli cb_h(CbPauli p,unsigned q){uint32_t b=cb_bit(q);unsigned x=(p.x>>q)&1u,z=(p.z>>q)&1u;q_count(5,4);p.k=(p.k+((x&z)<<1))&3u;q_count(5,4);if(x!=z){p.x^=b;p.z^=b;q_count(5,2);}return p;}
static CbPauli cb_s(CbPauli p,unsigned q){unsigned x=(p.x>>q)&1u;q_count(5,2);p.k=(p.k+x)&3u;q_count(5,2);if(x){p.z^=cb_bit(q);q_count(5,1);}return p;}
static CbPauli cb_cx(CbPauli p,unsigned c,unsigned t){unsigned x=(p.x>>c)&1u,z=(p.z>>t)&1u;q_count(5,4);if(x){p.x^=cb_bit(t);q_count(5,1);}if(z){p.z^=cb_bit(c);q_count(5,1);}return p;}
static uint32_t cb_row_mask(const uint8_t *rows,unsigned r){uint32_t z=0;for(unsigned q=0;q<24;q++){q_count(4,1);if(rows[q]==r){z|=cb_bit(q);q_count(5,1);}}return z;}
static uint32_t cb_col_mask(const uint8_t *cols,unsigned c){uint32_t x=0;for(unsigned q=0;q<24;q++){q_count(4,1);if(cols[q]==c){x|=cb_bit(q);q_count(5,1);}}return x;}
static void cb_declare(CbPauli *s,CbPauli *l,uint8_t *rows,uint8_t *cols){
 /* Literal site authority, independently encoded from DECLARED.json. */
 rows[0]=1;cols[0]=1;rows[1]=1;cols[1]=3;rows[2]=1;cols[2]=5;
 rows[3]=2;cols[3]=2;rows[4]=2;cols[4]=3;rows[5]=2;cols[5]=6;
 rows[6]=3;cols[6]=1;rows[7]=3;cols[7]=2;rows[8]=3;cols[8]=5;rows[9]=3;cols[9]=6;
 rows[10]=4;cols[10]=4;rows[11]=4;cols[11]=5;rows[12]=4;cols[12]=6;
 rows[13]=5;cols[13]=1;rows[14]=5;cols[14]=3;rows[15]=5;cols[15]=4;rows[16]=5;cols[16]=6;
 rows[17]=6;cols[17]=2;rows[18]=6;cols[18]=3;rows[19]=6;cols[19]=4;rows[20]=6;cols[20]=5;
 rows[21]=7;cols[21]=1;rows[22]=7;cols[22]=2;rows[23]=7;cols[23]=4;q_count(4,48);
 unsigned n=0;for(unsigned r=1;r<8;r++){unsigned prev=25;for(unsigned q=0;q<24;q++){q_count(4,1);if(rows[q]!=r)continue;if(prev<24){s[n]=cb_pos(cb_bit(prev)|cb_bit(q),0);q_count(5,1);q_count(4,1);n++;q_count(5,1);}prev=q;}}
 q_guard(n==17,"C seventeen declared horizontal rows");s[17]=cb_pos(0,UINT32_C(0xffffff));q_count(4,1);
 for(unsigned j=0;j<3;j++){uint32_t z=0;for(unsigned q=0;q<24;q++){q_count(4,1);unsigned row=(unsigned)rows[q];q_count(5,1);if((row>>j)&1u){z|=cb_bit(q);q_count(5,1);}q_count(5,2);}s[18+j]=cb_pos(0,z);q_count(4,1);q_count(5,1);}
 l[0]=cb_pos(0,cb_row_mask(rows,4));l[1]=cb_pos(0,cb_row_mask(rows,3));l[2]=cb_pos(0,cb_row_mask(rows,1));
 l[3]=cb_pos(cb_col_mask(cols,4),0);l[4]=cb_pos(cb_col_mask(cols,2),0);l[5]=cb_pos(cb_col_mask(cols,3),0);q_count(4,6);
}
static uint32_t cb_syndrome(CbPauli p,const CbPauli*s){uint32_t y=0;for(unsigned j=0;j<21;j++){q_count(4,1);y|=cb_anti(p,s[j])<<j;q_count(5,2);}return y;}
static unsigned cb_class(CbPauli p,const CbPauli*l){unsigned out=0;for(unsigned j=0;j<3;j++){q_count(4,2);unsigned x=cb_anti(p,l[3+j]),z=cb_anti(p,l[j]);unsigned a=x?(z?2u:1u):(z?3u:0u);out|=a<<(2*j);q_count(5,3);}return out;}
static CbPauli cb_logical(const CbPauli*l,unsigned label){CbPauli out={0,0,0};for(unsigned j=0;j<3;j++){unsigned a=(label>>(2*j))&3u;q_count(5,3);if(a==1){out=cb_mul(out,l[j]);q_count(4,1);}else if(a==3){out=cb_mul(out,l[3+j]);q_count(4,1);}else if(a==2){CbPauli y=cb_mul(l[j],l[3+j]);q_count(4,2);y.k=(y.k+1u)&3u;q_count(5,2);out=cb_mul(out,y);}}return out;}
static unsigned cb_top(uint64_t x){q_guard(x!=0,"C nonzero pivot coordinate");q_count(5,3);return 63u-(unsigned)__builtin_clzll(x);}
static void cb_basis(const CbPauli*s,uint64_t a[50][2]){memset(a,0,800);for(unsigned j=0;j<21;j++){q_count(4,1);CbPauli p=s[j];uint32_t mask=cb_bit(j);uint64_t coord=(uint64_t)p.x|((uint64_t)p.z<<24);q_count(5,4);while(coord){unsigned h=cb_top(coord);uint64_t*row=a[h];q_count(4,1);if(!row[0]){row[0]=cb_pack(p);row[1]=mask;break;}p=cb_mul(p,cb_unpack(row[0]));mask^=(uint32_t)row[1];q_count(5,2);coord=(uint64_t)p.x|((uint64_t)p.z<<24);q_count(5,4);}q_guard(coord!=0,"C independent signed stabilizer rank21");}}
static void cb_in_stabilizer(CbPauli p,uint64_t a[50][2]){uint64_t coord=(uint64_t)p.x|((uint64_t)p.z<<24);q_count(5,4);while(coord){unsigned h=cb_top(coord);uint64_t*row=a[h];q_count(4,1);q_guard(row[0]!=0,"C signed normalizer image has declared stabilizer witness");p=cb_mul(p,cb_unpack(row[0]));coord=(uint64_t)p.x|((uint64_t)p.z<<24);q_count(5,4);}q_guard(p.k==0,"C exact positive signed stabilizer image");}
static unsigned cb_code_row_map(const uint8_t*rows,const uint32_t*wire,const CbPauli*s,unsigned e){CbPauli f=cb_pos(0,0);unsigned xr=0;for(unsigned r=1;r<8;r++){if((e>>(r-1u))&1u){xr^=r;for(unsigned q=0;q<24;q++){q_count(4,1);if(rows[q]==r){f.x^=wire[q];q_count(4,1);q_count(5,1);break;}}q_count(5,1);}q_count(5,3);}unsigned t=(cb_pop(e)&1u)|(xr<<1);q_count(5,3);unsigned q=(((e>>3)^(e>>2))&1u)|((e&1u)<<1);q_count(5,7);uint32_t sy=cb_syndrome(f,s);q_guard(sy==(t<<17),"C full21-syndrome row boundary");q_count(5,3);return t|(q<<4);}
static unsigned cb_fast_row_map(unsigned e){unsigned xr=0;for(unsigned r=1;r<8;r++){if((e>>(r-1u))&1u){xr^=r;q_count(5,1);}q_count(5,3);}unsigned t=(cb_pop(e)&1u)|(xr<<1);q_count(5,3);unsigned q=(((e>>3)^(e>>2))&1u)|((e&1u)<<1);q_count(5,9);return t|(q<<4);}
#endif
