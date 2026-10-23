module Data exposing
    ( Dataset
    , bars
    , candles
    , forType
    , gaps
    , hello
    , isoTime
    , sample
    , series
    , spark
    )

{-| Seeded generated datasets for the chart pages and the doc pages. Every
one is simulated, and the pages say so with the seed (REQ-SITE-110). A
dataset is JSON text in the element's columns shorthand (REQ-DATA-120), set
as the `data` attribute.

The generator is xorshift32 (Marsaglia 2003) on 32-bit integers, so the same
seed gives the same data in every browser.

-}

import Bitwise
import Json.Decode as D
import Json.Encode as E
import Time


{-| JSON text for the `data` attribute, the seed it came from, and its size.
-}
type alias Dataset =
    { seed : Int
    , json : String
    , rows : Int
    , columns : List String
    }


{-| A chart type's dataset: a fixture-like set of the shape its meta takes.
-}
forType : String -> Dataset
forType t =
    case t of
        "candles" ->
            candles 2026 120

        "bar" ->
            bars 7

        "sparkline" ->
            spark 73 90

        _ ->
            series 11 240



-- THE GENERATOR


next : Int -> Int
next s0 =
    let
        s1 =
            Bitwise.xor s0 (Bitwise.shiftLeftBy 13 s0)

        s2 =
            Bitwise.xor s1 (Bitwise.shiftRightZfBy 17 s1)

        s3 =
            Bitwise.xor s2 (Bitwise.shiftLeftBy 5 s2)
    in
    Bitwise.shiftRightZfBy 0 s3


{-| Uniform in [0, 1), and the next state.
-}
uniform : Int -> ( Float, Int )
uniform s =
    let
        n =
            next s
    in
    ( toFloat n / 4294967296, n )


start : Int -> Int
start seed =
    if Bitwise.shiftRightZfBy 0 seed == 0 then
        2463534242

    else
        Bitwise.shiftRightZfBy 0 seed


round2 : Float -> Float
round2 v =
    toFloat (round (v * 100)) / 100


round1 : Float -> Float
round1 v =
    toFloat (round (v * 10)) / 10



-- TIME


day : Int
day =
    86400000


{-| 2025-01-01T00:00:00Z in epoch ms.
-}
jan1 : Int
jan1 =
    1735689600000


{-| An instant as RFC 3339 text in UTC, which the shell infers as a timestamp column.
-}
isoTime : Int -> String
isoTime ms =
    let
        p =
            Time.millisToPosix ms

        two n =
            String.padLeft 2 '0' (String.fromInt n)
    in
    String.fromInt (Time.toYear Time.utc p)
        ++ "-"
        ++ two (monthNumber (Time.toMonth Time.utc p))
        ++ "-"
        ++ two (Time.toDay Time.utc p)
        ++ "T"
        ++ two (Time.toHour Time.utc p)
        ++ ":"
        ++ two (Time.toMinute Time.utc p)
        ++ ":"
        ++ two (Time.toSecond Time.utc p)
        ++ "Z"


monthNumber : Time.Month -> Int
monthNumber m =
    case m of
        Time.Jan ->
            1

        Time.Feb ->
            2

        Time.Mar ->
            3

        Time.Apr ->
            4

        Time.May ->
            5

        Time.Jun ->
            6

        Time.Jul ->
            7

        Time.Aug ->
            8

        Time.Sep ->
            9

        Time.Oct ->
            10

        Time.Nov ->
            11

        Time.Dec ->
            12



-- DATASETS


columns : Int -> Int -> List ( String, E.Value ) -> Dataset
columns seed rows cols =
    { seed = seed
    , json = E.encode 0 (E.object cols)
    , rows = rows
    , columns = List.map Tuple.first cols
    }


{-| Three daily series from 2025-01-01: random walks on a seasonal wave.
-}
series : Int -> Int -> Dataset
series seed n =
    let
        names =
            [ ( "north", 12, 8 ), ( "south", 18, 6 ), ( "west", 6, 10 ) ]

        walkOf k ( _, base, amp ) ( acc, s ) =
            let
                ( values, s2 ) =
                    walk n s 1.2
            in
            ( List.indexedMap
                (\i w -> round1 (base + w + amp * sin (2 * pi * toFloat i / 120 + toFloat k)))
                values
                :: acc
            , s2
            )

        ( built, _ ) =
            List.foldl (\( k, spec ) st -> walkOf k spec st) ( [], start seed ) (List.indexedMap Tuple.pair names)

        times =
            List.map (\i -> isoTime (jan1 + i * day)) (List.range 0 (n - 1))
    in
    columns seed
        n
        (( "t", E.list E.string times )
            :: List.map2 (\( name, _, _ ) vs -> ( name, E.list E.float vs )) names (List.reverse built)
        )


{-| `n` steps of a walk from 0, each at most `size` either way.
-}
walk : Int -> Int -> Float -> ( List Float, Int )
walk n s0 size =
    let
        go i s w acc =
            if i >= n then
                ( List.reverse acc, s )

            else
                let
                    ( u, s2 ) =
                        uniform s

                    w2 =
                        w + (u - 0.5) * 2 * size
                in
                go (i + 1) s2 w2 (w2 :: acc)
    in
    go 0 s0 0 []


{-| Two series over x = 0 … n−1 for sparklines: a load between 5 and 95, and memory.
-}
spark : Int -> Int -> Dataset
spark seed n =
    let
        ( cpu, s1 ) =
            walk n (start seed) 6

        ( mem, _ ) =
            walk n s1 2
    in
    columns seed
        n
        [ ( "x", E.list E.int (List.range 0 (n - 1)) )
        , ( "cpu", E.list E.float (List.map (\w -> round1 (clamp 5 95 (50 + w))) cpu) )
        , ( "memory", E.list E.float (List.map (\w -> round1 (clamp 10 90 (60 + w))) mem) )
        ]


{-| `n` daily candles on weekdays from Monday 2025-01-06, with volume.
-}
candles : Int -> Int -> Dataset
candles seed n =
    let
        monday =
            jan1 + 5 * day

        weekday i =
            monday + ((i // 5) * 7 + modBy 5 i) * day

        go i s close acc =
            if i >= n then
                List.reverse acc

            else
                let
                    ( u1, s1 ) =
                        uniform s

                    ( u2, s2 ) =
                        uniform s1

                    ( u3, s3 ) =
                        uniform s2

                    ( u4, s4 ) =
                        uniform s3

                    open =
                        close

                    c =
                        round2 (open * (1 + (u1 - 0.49) * 0.05))

                    high =
                        round2 (max open c * (1 + u2 * 0.015))

                    low =
                        round2 (min open c * (1 - u3 * 0.015))

                    volume =
                        1000 + round (u4 * 4000)
                in
                go (i + 1) s4 c (( weekday i, ( open, high ), ( low, c, volume ) ) :: acc)

        rows =
            go 0 (start seed) 100 []
    in
    columns seed
        n
        [ ( "t", E.list (\( t, _, _ ) -> E.string (isoTime t)) rows )
        , ( "open", E.list (\( _, ( o, _ ), _ ) -> E.float o) rows )
        , ( "high", E.list (\( _, ( _, h ), _ ) -> E.float h) rows )
        , ( "low", E.list (\( _, _, ( l, _, _ ) ) -> E.float l) rows )
        , ( "close", E.list (\( _, _, ( _, c, _ ) ) -> E.float c) rows )
        , ( "volume", E.list (\( _, _, ( _, _, v ) ) -> E.int v) rows )
        ]


{-| Plan against actual for six quarters, with a third series that changes sign.
-}
bars : Int -> Dataset
bars seed =
    let
        quarters =
            [ "2025 Q1", "2025 Q2", "2025 Q3", "2025 Q4", "2026 Q1", "2026 Q2" ]

        n =
            List.length quarters

        draw count s0 f =
            List.foldl
                (\i ( acc, s ) ->
                    let
                        ( u, s2 ) =
                            uniform s
                    in
                    ( f i u :: acc, s2 )
                )
                ( [], s0 )
                (List.range 0 (count - 1))
                |> Tuple.mapFirst List.reverse

        ( plan, s1 ) =
            draw n (start seed) (\i u -> round1 (40 + toFloat i * 4 + u * 6))

        ( actual, _ ) =
            draw n s1 (\i u -> round1 (36 + toFloat i * 5 + u * 12))

        change =
            List.map2 (\a p -> round1 (a - p)) actual plan
    in
    columns seed
        n
        [ ( "quarter", E.list E.string quarters )
        , ( "plan", E.list E.float plan )
        , ( "actual", E.list E.float actual )
        , ( "change", E.list E.float change )
        ]


{-| The getting-started chart: eight points, written out in the snippet beside it.
-}
hello : Dataset
hello =
    columns 0
        8
        [ ( "x", E.list E.int (List.range 0 7) )
        , ( "y", E.list E.float [ 3, 4.5, 4, 6.5, 6, 8, 7.5, 9 ] )
        ]


{-| A series with two missing values (null), to show a line breaking at a gap (REQ-DATA-030).
-}
gaps : Dataset
gaps =
    let
        ys =
            List.map
                (\i ->
                    if i == 9 || i == 10 || i == 22 then
                        E.null

                    else
                        E.float (round1 (10 + 6 * sin (toFloat i / 4) + toFloat i / 5))
                )
                (List.range 0 31)
    in
    { seed = 0
    , json = E.encode 0 (E.object [ ( "x", E.list E.int (List.range 0 31) ), ( "level", E.list identity ys ) ])
    , rows = 32
    , columns = [ "x", "level" ]
    }


{-| The first `n` rows of a dataset in the columns shorthand, as JSON text, for a snippet.
-}
sample : Int -> Dataset -> String
sample n dataset =
    case D.decodeString (D.keyValuePairs (D.list D.value)) dataset.json of
        Ok cols ->
            E.encode 0 (E.object (List.map (\( k, vs ) -> ( k, E.list identity (List.take n vs) )) cols))

        Err _ ->
            "{}"
