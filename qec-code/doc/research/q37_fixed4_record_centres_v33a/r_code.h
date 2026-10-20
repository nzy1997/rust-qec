/* Independent R literal code algebra. SOURCE TEXT ONLY; no authoring run.
 * Primary2610.06730v1 B/Table4; no C generator or header is an input. */
#ifndef Q37_R_CODE_FIXED4_V33_H
#define Q37_R_CODE_FIXED4_V33_H
#include <stdint.h>
#define R4_SITE(i) (UINT64_C(1)<<((i)-1))
static uint64_t r4_code_x(unsigned i){switch(i){
case 0:return R4_SITE(19)|R4_SITE(18);
case 1:return R4_SITE(33)|R4_SITE(11);
case 2:return R4_SITE(34)|R4_SITE(12);
case 3:return R4_SITE(13)|R4_SITE(35);
case 4:return R4_SITE(37)|R4_SITE(15);
case 5:return R4_SITE(3)|R4_SITE(21)|R4_SITE(18)|R4_SITE(2);
case 6:return R4_SITE(22)|R4_SITE(4)|R4_SITE(1)|R4_SITE(16);
case 7:return R4_SITE(17)|R4_SITE(26)|R4_SITE(6)|R4_SITE(1);
case 8:return R4_SITE(27)|R4_SITE(26)|R4_SITE(25)|R4_SITE(5);
case 9:return R4_SITE(7)|R4_SITE(28)|R4_SITE(5)|R4_SITE(24);
case 10:return R4_SITE(9)|R4_SITE(31)|R4_SITE(32)|R4_SITE(10);
case 11:return R4_SITE(30)|R4_SITE(8)|R4_SITE(14)|R4_SITE(36);
case 12:return R4_SITE(7)|R4_SITE(23)|R4_SITE(21)|R4_SITE(20)|R4_SITE(22)|R4_SITE(29);
case 13:return R4_SITE(9)|R4_SITE(31)|R4_SITE(25)|R4_SITE(24)|R4_SITE(36)|R4_SITE(14);
case 14:return R4_SITE(34)|R4_SITE(15)|R4_SITE(5)|R4_SITE(6)|R4_SITE(8)|R4_SITE(33)|R4_SITE(19)|R4_SITE(17);
case 15:return R4_SITE(3)|R4_SITE(22)|R4_SITE(12)|R4_SITE(10)|R4_SITE(2)|R4_SITE(24)|R4_SITE(33)|R4_SITE(35);
case 16:return R4_SITE(32)|R4_SITE(7)|R4_SITE(27)|R4_SITE(18)|R4_SITE(14)|R4_SITE(15)|R4_SITE(21)|R4_SITE(11);
case 17:return R4_SITE(13)|R4_SITE(9)|R4_SITE(19)|R4_SITE(26)|R4_SITE(12)|R4_SITE(21)|R4_SITE(30)|R4_SITE(28);
default:q_fail("R code X row");return 0;}}
static uint64_t r4_code_z(unsigned i){switch(i){
case 0:return R4_SITE(3)|R4_SITE(2);
case 1:return R4_SITE(16)|R4_SITE(4);
case 2:return R4_SITE(6)|R4_SITE(17);
case 3:return R4_SITE(23)|R4_SITE(20);
case 4:return R4_SITE(20)|R4_SITE(29);
case 5:return R4_SITE(9)|R4_SITE(28)|R4_SITE(24)|R4_SITE(10);
case 6:return R4_SITE(26)|R4_SITE(5)|R4_SITE(6)|R4_SITE(28);
case 7:return R4_SITE(9)|R4_SITE(32)|R4_SITE(30)|R4_SITE(14);
case 8:return R4_SITE(27)|R4_SITE(25)|R4_SITE(31)|R4_SITE(32);
case 9:return R4_SITE(7)|R4_SITE(25)|R4_SITE(14)|R4_SITE(23)|R4_SITE(5)|R4_SITE(8);
case 10:return R4_SITE(27)|R4_SITE(19)|R4_SITE(5)|R4_SITE(21)|R4_SITE(7)|R4_SITE(18);
case 11:return R4_SITE(7)|R4_SITE(16)|R4_SITE(25)|R4_SITE(24)|R4_SITE(27)|R4_SITE(22);
case 12:return R4_SITE(27)|R4_SITE(22)|R4_SITE(26)|R4_SITE(21)|R4_SITE(1)|R4_SITE(2);
case 13:return R4_SITE(31)|R4_SITE(11)|R4_SITE(14)|R4_SITE(8)|R4_SITE(33)|R4_SITE(10);
case 14:return R4_SITE(26)|R4_SITE(30)|R4_SITE(1)|R4_SITE(25)|R4_SITE(36)|R4_SITE(16);
case 15:return R4_SITE(34)|R4_SITE(9)|R4_SITE(10)|R4_SITE(12)|R4_SITE(8)|R4_SITE(36);
case 16:return R4_SITE(10)|R4_SITE(13)|R4_SITE(31)|R4_SITE(36)|R4_SITE(35)|R4_SITE(30);
case 17:return R4_SITE(32)|R4_SITE(37)|R4_SITE(31)|R4_SITE(8)|R4_SITE(15)|R4_SITE(36);
default:q_fail("R code Z row");return 0;}}
static uint64_t r4_code_lx(void){return R4_SITE(2)|R4_SITE(3)|R4_SITE(5)|R4_SITE(6)|R4_SITE(7)|R4_SITE(17)|R4_SITE(22);}
static uint64_t r4_code_lz(void){return R4_SITE(1)|R4_SITE(8)|R4_SITE(9)|R4_SITE(16)|R4_SITE(17)|R4_SITE(30)|R4_SITE(31);}
#undef R4_SITE

/* Code partition geometry is algebra; circuit lowering stays in reference.c. */
static unsigned r4_part(unsigned part,unsigned j){q_count(4,1);switch(part){
case 0:return j==0?0u:j==1?15u:16u;
case 1:return j==0?1u:j==1?17u:18u;
case 2:return j==0?2u:j==1?19u:20u;
case 3:return j==0?3u:j==1?21u:22u;
case 4:return j==0?4u:j==1?23u:24u;
case 5:return j==0?5u:j==1?25u:26u;
case 6:return j==0?6u:j==1?27u:28u;
case 7:return j?29u:7u;
case 8:return j?30u:8u;
case 9:return j?31u:9u;
case 10:return j?32u:10u;
case 11:return j?33u:11u;
case 12:return j?34u:12u;
case 13:return j?35u:13u;
case 14:return j?36u:14u;
default:q_fail("R4 part ordinal");return 0;}}
static unsigned r4_part_size(unsigned part){q_count(4,1);return part<7u?3u:2u;}

#endif
