module SiteTest exposing (suite)

{-| The site's pure parts: routes, option controls and the generated data.
-}

import Data
import Dict
import Expect
import Json.Decode as D
import Json.Encode as E
import Options exposing (Control(..))
import Route exposing (Route(..))
import Test exposing (Test, describe, test)
import Url


url : String -> Url.Url
url path =
    Url.fromString ("http://localhost" ++ path)
        |> Maybe.withDefault { protocol = Url.Http, host = "localhost", port_ = Nothing, path = "/", query = Nothing, fragment = Nothing }


routeOf : String -> String -> Route
routeOf base path =
    Route.fromUrl base (url path)


lineOptions : List ( String, D.Value )
lineOptions =
    D.decodeString (D.keyValuePairs D.value)
        """{"annotations":[],"baseline":0,"encode":{},"legend":{"position":"auto"},"mode":"line","spanGaps":false,"step":"after","title":""}"""
        |> Result.withDefault []


suite : Test
suite =
    describe "the site"
        [ describe "routes (REQ-SITE-010)"
            [ test "the front page" <|
                \_ -> routeOf "/" "/" |> Expect.equal Home
            , test "a chart type" <|
                \_ -> routeOf "/" "/candles" |> Expect.equal (ChartType "candles")
            , test "a doc page" <|
                \_ -> routeOf "/" "/docs/theming" |> Expect.equal (Doc "theming")
            , test "the gallery with filters" <|
                \_ ->
                    routeOf "/" "/examples?type=line&milestone=v0.7.0"
                        |> Expect.equal (Gallery { noFilters | type_ = Just "line", milestone = Just "v0.7.0" })
            , test "an example" <|
                \_ -> routeOf "/" "/examples/hello-line" |> Expect.equal (Example "hello-line")
            , test "under a base folder" <|
                \_ -> routeOf "/glyphplot/" "/glyphplot/docs/cli" |> Expect.equal (Doc "cli")
            , test "outside the base folder" <|
                \_ -> routeOf "/glyphplot/" "/docs/cli" |> Expect.equal NotFound
            , test "a relative base is the root" <|
                \_ -> Route.toPath "./" (Example "a") |> Expect.equal "/examples/a"
            , test "paths round-trip" <|
                \_ ->
                    [ Home, ChartType "bar", Doc "data", Gallery { noFilters | feature = Just "streaming" }, Example "x" ]
                        |> List.map (\r -> routeOf "/site/" (Route.toPath "/site/" r) == r)
                        |> Expect.equal [ True, True, True, True, True ]
            ]
        , describe "option controls (REQ-SITE-020)"
            [ test "a control per option, mode first, objects of plain values split" <|
                \_ ->
                    Options.fields [ "line", "step", "area" ] lineOptions
                        |> List.map (\f -> ( f.path, f.control ))
                        |> Expect.equal
                            [ ( "mode", Choice [ "line", "step", "area" ] )
                            , ( "annotations", JsonText )
                            , ( "baseline", Number )
                            , ( "encode", JsonText )
                            , ( "legend.position", Choice [ "auto", "outside", "inside", "none" ] )
                            , ( "spanGaps", Toggle )
                            , ( "step", Choice [ "after", "before", "middle" ] )
                            , ( "title", Text )
                            ]
            , test "only changed options, nested back by path" <|
                \_ ->
                    Options.overridesJson (Options.fields [ "line" ] lineOptions)
                        (Dict.fromList [ ( "legend.position", "none" ), ( "spanGaps", "true" ), ( "baseline", "0" ), ( "step", "after" ) ])
                        |> Expect.equal """{"legend":{"position":"none"},"spanGaps":true}"""
            , test "no change is an empty object" <|
                \_ -> Options.overridesJson (Options.fields [ "line" ] lineOptions) Dict.empty |> Expect.equal "{}"
            , test "text that does not parse is reported and left out" <|
                \_ ->
                    let
                        fields =
                            Options.fields [ "line" ] lineOptions

                        inputs =
                            Dict.fromList [ ( "baseline", "abc" ), ( "annotations", "[{" ) ]
                    in
                    ( Options.problems fields inputs, Options.overridesJson fields inputs )
                        |> Expect.equal ( [ "annotations: not JSON", "baseline: not a number" ], "{}" )
            ]
        , describe "generated data (REQ-SITE-110)"
            [ test "the same seed, the same data" <|
                \_ -> (Data.series 11 50).json |> Expect.equal (Data.series 11 50).json
            , test "another seed, other data" <|
                \_ -> (Data.series 11 50).json |> Expect.notEqual (Data.series 12 50).json
            , test "every column as long as the rows" <|
                \_ ->
                    [ Data.series 1 40, Data.spark 2 30, Data.candles 3 25, Data.bars 4 ]
                        |> List.map (\d -> D.decodeString (D.keyValuePairs (D.list D.value)) d.json |> Result.map (List.all (\( _, vs ) -> List.length vs == d.rows)))
                        |> Expect.equal [ Ok True, Ok True, Ok True, Ok True ]
            , test "times are RFC 3339 in UTC" <|
                \_ -> Data.isoTime 1735776000000 |> Expect.equal "2025-01-02T00:00:00Z"
            , test "a snippet's sample is the first rows" <|
                \_ -> Data.sample 2 Data.hello |> Expect.equal (E.encode 0 (E.object [ ( "x", E.list E.int [ 0, 1 ] ), ( "y", E.list E.float [ 3, 4.5 ] ) ]))
            ]
        ]


noFilters : Route.Filters
noFilters =
    Route.noFilters
