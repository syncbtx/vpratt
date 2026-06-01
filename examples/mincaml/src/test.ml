-- let (a, b, c) = 5 in x + x
-- let x = Array.create 10 0 in
-- let x = f x y in x.(0) <- 10 ; x > 3 ; x +. 4 ; x <= y

-- let rec scale x y z s =
   -- (x *. s, y *. s, z *. s)
-- in x

let rec ackermann m n =
    if m <= 0 then n + 1
    else if n <= 0 then ackermann (m - 1) 1
    else ackermann (m - 1) (ackermann m (n - 1))
in

let (x, y) = (1, -2) in
let arr = Array.create 10 (x + y) in

let rec simulate i v =
    if i >= 10 then ()
    else (
        arr.(i) <- arr.(i - 1) + v ;
        simulate (i + 1) (-v)
    )
in

let final_state =
    arr.(0) <- x ;
    arr.(1) <- y * 3 ;
    simulate 2 1 ;

    let (fst, snd) = (arr.(8), arr.(9)) in
    if fst = snd then
        ackermann fst snd
    else
        ackermann x y
    in (final_state,arr.(5))