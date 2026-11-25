-- Swap name_en and name_ja on the three starter rows seeded backwards in 0012,
-- and remove localize-generated tag translations that wrote the Japanese name
-- as English so localize regenerates them with the romaji.

update avatars
   set name_en = name_ja,
       name_ja = name_en
 where (slug = 'pon-chune' and name_en = 'ぽんとちゅね' and name_ja = 'Pon & Chune')
    or (slug = 'ragussiel' and name_en = 'ラグシエル' and name_ja = 'Ragussiel')
    or (slug = 'tien' and name_en = 'ティエン' and name_ja = 'Tien');

delete from booth_tag_translations
 where source = 'localize'
   and en in ('ぽんとちゅね', 'ラグシエル', 'ティエン');
