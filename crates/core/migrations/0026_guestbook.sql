-- The guestbook: anonymous messages behind a Turnstile check (crates/api/src/guestbook.rs).

-- Whether `t` holds a payment card number: 13 to 19 digits that pass the Luhn check, inside a run of digit groups
-- joined by single spaces, hyphens or dots. The number may be any stretch of whole groups in the run, so a CVV or
-- expiry written after it does not hide it. The API refuses such text first (`guestbook::has_card_number`); this is
-- the backstop. A stretch stops growing past 19 digits: a 500-character message of one-digit groups takes about 6 ms,
-- where trying every stretch took 5 s.
create function has_card_number(t text) returns boolean
language plpgsql immutable parallel safe as $$
declare
  run text;
  g text[];
  d text;
  s int;
  x int;
begin
  for run in select m[1] from regexp_matches(t, '[0-9]+(?:[ .-][0-9]+)*', 'g') m loop
    g := regexp_split_to_array(run, '[ .-]');
    for a in 1 .. cardinality(g) loop
      d := '';
      for b in a .. cardinality(g) loop
        d := d || g[b];
        exit when length(d) > 19;
        if length(d) >= 13 then
          s := 0;
          for k in 1 .. length(d) loop
            x := substr(d, length(d) - k + 1, 1)::int;
            if k % 2 = 0 then
              x := x * 2;
              if x > 9 then x := x - 9; end if;
            end if;
            s := s + x;
          end loop;
          if s % 10 = 0 then return true; end if;
        end if;
      end loop;
    end loop;
  end loop;
  return false;
end
$$;

create table guestbook (
  id bigint generated always as identity primary key,
  nickname text not null check (char_length(nickname) between 1 and 24 and not has_card_number(nickname)),
  body text not null check (char_length(body) between 1 and 500 and not has_card_number(body)),
  created_at timestamptz not null default now(),
  hidden_at timestamptz
);

create index guestbook_shown on guestbook (id desc) where hidden_at is null;
