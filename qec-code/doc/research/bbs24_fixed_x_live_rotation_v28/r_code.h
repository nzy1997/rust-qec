/* Independently encoded R algebra export. SOURCE TEXT, not executed at authoring.
   Authority: DECLARED.json bd30f715fa96a43623354b432f4c44862fb01b7846bee02bbe8efe746f3447ac.
   Code declarations/qualification stay upstream; no circuit generator here. */
#ifndef R_BBS24_CODE_V28_H
#define R_BBS24_CODE_V28_H
#include <stdint.h>
typedef struct {uint32_t x,z;uint8_t sign,pad[3];} RCodeLiteral;
typedef struct {uint8_t site[24][2];RCodeLiteral stabilizer[21],logical[6],body[9];uint8_t label[9];} RCodeDeclaration;
#define R_CB(n) (UINT32_C(1)<<(n))
static const RCodeDeclaration r_code_declaration={
 {{1,1},{1,3},{1,5},{2,2},{2,3},{2,6},{3,1},{3,2},{3,5},{3,6},
  {4,4},{4,5},{4,6},{5,1},{5,3},{5,4},{5,6},{6,2},{6,3},{6,4},{6,5},{7,1},{7,2},{7,4}},
 {
 {R_CB(0)|R_CB(1),0,0,{0,0,0}},{R_CB(1)|R_CB(2),0,0,{0,0,0}},
 {R_CB(3)|R_CB(4),0,0,{0,0,0}},{R_CB(4)|R_CB(5),0,0,{0,0,0}},
 {R_CB(6)|R_CB(7),0,0,{0,0,0}},{R_CB(7)|R_CB(8),0,0,{0,0,0}},{R_CB(8)|R_CB(9),0,0,{0,0,0}},
 {R_CB(10)|R_CB(11),0,0,{0,0,0}},{R_CB(11)|R_CB(12),0,0,{0,0,0}},
 {R_CB(13)|R_CB(14),0,0,{0,0,0}},{R_CB(14)|R_CB(15),0,0,{0,0,0}},{R_CB(15)|R_CB(16),0,0,{0,0,0}},
 {R_CB(17)|R_CB(18),0,0,{0,0,0}},{R_CB(18)|R_CB(19),0,0,{0,0,0}},{R_CB(19)|R_CB(20),0,0,{0,0,0}},
 {R_CB(21)|R_CB(22),0,0,{0,0,0}},{R_CB(22)|R_CB(23),0,0,{0,0,0}},
 {0,UINT32_C(0xffffff),0,{0,0,0}},
 {0,R_CB(0)|R_CB(1)|R_CB(2)|R_CB(6)|R_CB(7)|R_CB(8)|R_CB(9)|R_CB(13)|R_CB(14)|R_CB(15)|R_CB(16)|R_CB(21)|R_CB(22)|R_CB(23),0,{0,0,0}},
 {0,R_CB(3)|R_CB(4)|R_CB(5)|R_CB(6)|R_CB(7)|R_CB(8)|R_CB(9)|R_CB(17)|R_CB(18)|R_CB(19)|R_CB(20)|R_CB(21)|R_CB(22)|R_CB(23),0,{0,0,0}},
 {0,R_CB(10)|R_CB(11)|R_CB(12)|R_CB(13)|R_CB(14)|R_CB(15)|R_CB(16)|R_CB(17)|R_CB(18)|R_CB(19)|R_CB(20)|R_CB(21)|R_CB(22)|R_CB(23),0,{0,0,0}}
 },
 {
 {0,R_CB(10)|R_CB(11)|R_CB(12),0,{0,0,0}},
 {0,R_CB(6)|R_CB(7)|R_CB(8)|R_CB(9),0,{0,0,0}},
 {0,R_CB(0)|R_CB(1)|R_CB(2),0,{0,0,0}},
 {R_CB(10)|R_CB(15)|R_CB(19)|R_CB(23),0,0,{0,0,0}},
 {R_CB(3)|R_CB(7)|R_CB(17)|R_CB(22),0,0,{0,0,0}},
 {R_CB(1)|R_CB(4)|R_CB(14)|R_CB(18),0,0,{0,0,0}}
 },
 {
 {0,R_CB(10)|R_CB(11)|R_CB(12),0,{0,0,0}},
 {0,R_CB(6)|R_CB(7)|R_CB(8)|R_CB(9),0,{0,0,0}},
 {0,R_CB(0)|R_CB(1)|R_CB(2),0,{0,0,0}},
 {0,R_CB(21)|R_CB(22)|R_CB(23),0,{0,0,0}},
 {0,R_CB(13)|R_CB(14)|R_CB(15)|R_CB(16),0,{0,0,0}},
 {0,R_CB(17)|R_CB(18)|R_CB(19)|R_CB(20),0,{0,0,0}},
 {0,R_CB(3)|R_CB(4)|R_CB(5),0,{0,0,0}},
 {R_CB(5)|R_CB(9)|R_CB(12)|R_CB(16),R_CB(21)|R_CB(22)|R_CB(23),1,{0,0,0}},
 {R_CB(5)|R_CB(9)|R_CB(12)|R_CB(16),R_CB(17)|R_CB(18)|R_CB(19)|R_CB(20),1,{0,0,0}}
 },{1,4,16,5,17,21,20,10,26}
};
#undef R_CB
typedef struct {uint32_t x,z,k;} RCodePhase;
typedef struct {RCodePhase basis[48],a,b,c;uint32_t rank,pivot,row,bit,i,j,word;} RCodeWork;
/* Explicit accounting is supplied by a verifier; these routines are algebra
   export qualification, not a native-circuit generator. */
#ifndef R_ALG_COUNT
#error R_ALG_COUNT_must_be_bound_to_original_counter
#endif
#ifndef R_ALG_GUARD
#error R_ALG_GUARD_must_be_bound_to_verifier_guard
#endif
static unsigned r_code_parity(uint32_t v){unsigned p=0;while(v){v&=v-1;p^=1;R_ALG_COUNT(5,3);}return p;}
static unsigned r_code_pop(uint32_t v){unsigned p=0;while(v){v&=v-1;p++;R_ALG_COUNT(5,3);}return p;}
static RCodePhase r_code_phase(RCodeLiteral p){RCodePhase v={p.x,p.z,0};v.k=(r_code_pop(p.x&p.z)+((unsigned)p.sign<<1))&3u;R_ALG_COUNT(5,5);return v;}
static RCodePhase r_code_product(RCodePhase a,RCodePhase b){RCodePhase c;c.k=(a.k+b.k+(r_code_parity(a.z&b.x)<<1))&3u;c.x=a.x^b.x;c.z=a.z^b.z;R_ALG_COUNT(5,7);return c;}
static unsigned r_code_anti(RCodePhase a,RCodePhase b){R_ALG_COUNT(5,3);return r_code_parity((a.x&b.z)^(a.z&b.x));}
static unsigned r_code_coordinate(RCodePhase p,unsigned bit){R_ALG_COUNT(4,1);if(bit<24){R_ALG_COUNT(5,2);return(p.x>>bit)&1u;}R_ALG_COUNT(5,3);return(p.z>>(bit-24))&1u;}
static void r_code_insert(RCodeWork*w,RCodePhase p){w->a=p;for(w->bit=0;w->bit<48;w->bit++){if(!r_code_coordinate(w->a,w->bit))continue;R_ALG_COUNT(4,1);if(w->basis[w->bit].x||w->basis[w->bit].z)w->a=r_code_product(w->a,w->basis[w->bit]);else{w->basis[w->bit]=w->a;w->rank++;R_ALG_COUNT(5,1);return;}}R_ALG_GUARD(!w->a.k,"r_code_negative_identity");}
static void r_code_reduce(RCodeWork*w,RCodePhase p){w->a=p;for(w->bit=0;w->bit<48;w->bit++)if(r_code_coordinate(w->a,w->bit)){R_ALG_COUNT(4,1);R_ALG_GUARD(w->basis[w->bit].x||w->basis[w->bit].z,"r_code_nonstabilizer_target");w->a=r_code_product(w->a,w->basis[w->bit]);}R_ALG_GUARD(!w->a.x&&!w->a.z&&!w->a.k,"r_code_signed_target");}
static void r_code_qualify(RCodeWork*w){
 for(w->i=0;w->i<21;w->i++)for(w->j=0;w->j<21;w->j++){R_ALG_COUNT(4,2);R_ALG_GUARD(!r_code_anti(r_code_phase(r_code_declaration.stabilizer[w->i]),r_code_phase(r_code_declaration.stabilizer[w->j])),"r_code_stabilizer_commutation");}
 for(w->i=0;w->i<6;w->i++){
  for(w->j=0;w->j<21;w->j++){R_ALG_COUNT(4,2);R_ALG_GUARD(!r_code_anti(r_code_phase(r_code_declaration.logical[w->i]),r_code_phase(r_code_declaration.stabilizer[w->j])),"r_code_logical_normalizer");}
  for(w->j=0;w->j<6;w->j++){unsigned partner=w->i<3?w->i+3:w->i-3;R_ALG_COUNT(5,1);unsigned expected=partner==w->j;R_ALG_COUNT(5,1);R_ALG_COUNT(4,2);R_ALG_GUARD(r_code_anti(r_code_phase(r_code_declaration.logical[w->i]),r_code_phase(r_code_declaration.logical[w->j]))==expected,"r_code_logical_pairs");}
 }
 for(w->i=0;w->i<48;w->i++)w->basis[w->i]=(RCodePhase){0,0,0};w->rank=0;
 for(w->row=0;w->row<21;w->row++){R_ALG_COUNT(4,1);r_code_insert(w,r_code_phase(r_code_declaration.stabilizer[w->row]));}R_ALG_GUARD(w->rank==21,"r_code_rank21");
 for(w->word=0;w->word<9;w->word++){
  w->c=(RCodePhase){0,0,0};
  for(w->j=0;w->j<3;w->j++){unsigned d=((unsigned)r_code_declaration.label[w->word]>>(2*w->j))&3u;R_ALG_COUNT(4,1);R_ALG_COUNT(5,4);
   if(d==1||d==2){R_ALG_COUNT(4,1);w->c=r_code_product(w->c,r_code_phase(r_code_declaration.logical[w->j]));}
   if(d==2||d==3){R_ALG_COUNT(4,1);R_ALG_COUNT(5,1);w->c=r_code_product(w->c,r_code_phase(r_code_declaration.logical[w->j+3]));}
   if(d==2){w->c.k=(w->c.k+1)&3u;R_ALG_COUNT(5,2);}
  }
  R_ALG_COUNT(4,1);w->b=r_code_product(r_code_phase(r_code_declaration.body[w->word]),w->c);r_code_reduce(w,w->b);
 }
}
static unsigned r_code_rowmask(uint32_t x){unsigned e=0;for(unsigned i=0;i<24;i++){R_ALG_COUNT(4,2);uint32_t b=UINT32_C(1)<<i;R_ALG_COUNT(5,2);if(x&b){unsigned row=(unsigned)r_code_declaration.site[i][0];R_ALG_COUNT(5,1);row--;R_ALG_COUNT(5,1);e^=1u<<row;R_ALG_COUNT(5,2);}}return e;}
static unsigned r_code_t(unsigned e){unsigned t=r_code_parity(e),v=0;for(unsigned row=1;row<=7;row++){R_ALG_COUNT(4,1);R_ALG_COUNT(5,3);if((e>>(row-1))&1u){v^=row;R_ALG_COUNT(5,1);}}R_ALG_COUNT(5,2);return t|(v<<1);}
static unsigned r_code_q(unsigned e){unsigned a=(e>>3)&1u,b=(e>>2)&1u,c=e&1u;R_ALG_COUNT(5,8);return(a^b)|(c<<1);}
#endif
