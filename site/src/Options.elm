module Options exposing
    ( Control(..)
    , Field
    , Inputs
    , fields
    , inputOf
    , overrides
    , overridesJson
    , problems
    )

{-| A control per option (REQ-SITE-020), made from a chart's meta: its
options' defaults say each option's kind. A boolean is a checkbox, a number a
number input, a string with a known set of values a select, any other string
a text input, and an object of plain values a control per key (`legend.position`,
`retain.rows`). An empty object or a list (`encode`, `annotations`) is JSON
text.

The sets of values are the ones the charts accept: `mode` from the meta's
modes, the rest as `crates/glyphplot/src/charts/*/mod.rs` reads them with
`choice` and spec/schemas/options.schema.json lists them.

-}

import Dict exposing (Dict)
import Json.Decode as D
import Json.Encode as E exposing (Value)


type Control
    = Choice (List String)
    | Toggle
    | Number
    | Text
    | JsonText


{-| One control: its dotted path, its default and its kind.
-}
type alias Field =
    { path : String
    , default : Value
    , control : Control
    }


{-| What the reader typed or chose, by path; a path not here is at its default.
-}
type alias Inputs =
    Dict String String


{-| The sets of values of the string options, by dotted path.
-}
choices : Dict String (List String)
choices =
    Dict.fromList
        [ ( "step", [ "after", "before", "middle" ] )
        , ( "orient", [ "vertical", "horizontal" ] )
        , ( "signed", [ "auto", "on", "off" ] )
        , ( "drag", [ "pan", "brush", "zoom" ] )
        , ( "wheel", [ "scroll", "zoom" ] )
        , ( "patterns", [ "auto", "on", "off" ] )
        , ( "xScale", [ "index", "time" ] )
        , ( "snap", [ "auto", "rows", "none" ] )
        , ( "sample", [ "m4", "lttb", "none" ] )
        , ( "legend.position", [ "auto", "outside", "inside", "none" ] )
        ]


{-| The controls for a meta's options and modes, `mode` first.
-}
fields : List String -> List ( String, Value ) -> List Field
fields modes options =
    let
        all =
            List.concatMap (\( key, v ) -> fieldsOf modes key v) options
    in
    List.filter (\f -> f.path == "mode") all ++ List.filter (\f -> f.path /= "mode") all


fieldsOf : List String -> String -> Value -> List Field
fieldsOf modes path v =
    case D.decodeValue kind v of
        Ok (Scalar control) ->
            [ { path = path
              , default = v
              , control =
                    case control of
                        Text ->
                            if path == "mode" then
                                Choice modes

                            else
                                Dict.get path choices |> Maybe.map Choice |> Maybe.withDefault Text

                        _ ->
                            control
              }
            ]

        Ok (Object pairs) ->
            if List.isEmpty pairs || List.any (\( _, x ) -> not (isScalar x)) pairs then
                [ { path = path, default = v, control = JsonText } ]

            else
                List.concatMap (\( key, x ) -> fieldsOf modes (path ++ "." ++ key) x) pairs

        Ok Other ->
            [ { path = path, default = v, control = JsonText } ]

        Err _ ->
            [ { path = path, default = v, control = JsonText } ]


type Kind
    = Scalar Control
    | Object (List ( String, Value ))
    | Other


kind : D.Decoder Kind
kind =
    D.oneOf
        [ D.bool |> D.map (\_ -> Scalar Toggle)
        , D.float |> D.map (\_ -> Scalar Number)
        , D.string |> D.map (\_ -> Scalar Text)
        , D.keyValuePairs D.value |> D.map Object
        , D.succeed Other
        ]


isScalar : Value -> Bool
isScalar v =
    case D.decodeValue kind v of
        Ok (Scalar _) ->
            True

        _ ->
            False


{-| The text a control shows: the reader's input, else the default's.
-}
inputOf : Inputs -> Field -> String
inputOf inputs f =
    case Dict.get f.path inputs of
        Just s ->
            s

        Nothing ->
            defaultText f


defaultText : Field -> String
defaultText f =
    case f.control of
        JsonText ->
            E.encode 0 f.default

        _ ->
            case D.decodeValue D.string f.default of
                Ok s ->
                    s

                Err _ ->
                    E.encode 0 f.default


{-| A control's value as JSON, or an error for text that does not parse.
-}
valueOf : Field -> String -> Result String Value
valueOf f text =
    case f.control of
        Toggle ->
            Ok (E.bool (text == "true"))

        Number ->
            case String.toFloat (String.trim text) of
                Just n ->
                    Ok (E.float n)

                Nothing ->
                    Err (f.path ++ ": not a number")

        JsonText ->
            D.decodeString D.value text |> Result.mapError (\_ -> f.path ++ ": not JSON")

        _ ->
            Ok (E.string text)


{-| The options that differ from their defaults, as (path, value), skipping inputs that do not parse.
-}
overrides : List Field -> Inputs -> List ( String, Value )
overrides fs inputs =
    List.filterMap
        (\f ->
            Dict.get f.path inputs
                |> Maybe.andThen (valueOf f >> Result.toMaybe)
                |> Maybe.andThen
                    (\v ->
                        if E.encode 0 v == E.encode 0 f.default then
                            Nothing

                        else
                            Just ( f.path, v )
                    )
        )
        fs


{-| The inputs that do not parse, as messages.
-}
problems : List Field -> Inputs -> List String
problems fs inputs =
    List.filterMap
        (\f ->
            Dict.get f.path inputs
                |> Maybe.andThen
                    (\text ->
                        case valueOf f text of
                            Err e ->
                                Just e

                            Ok _ ->
                                Nothing
                    )
        )
        fs


{-| The `options` attribute's JSON text: the overrides, nested back by path (`{"legend":{"position":"none"}}`); `{}` when none.
-}
overridesJson : List Field -> Inputs -> String
overridesJson fs inputs =
    E.encode 0 (nest (overrides fs inputs))


nest : List ( String, Value ) -> Value
nest pairs =
    let
        split ( path, v ) =
            case String.split "." path of
                [] ->
                    ( path, [], v )

                first :: rest ->
                    ( first, rest, v )

        grouped =
            List.foldr
                (\pair acc ->
                    let
                        ( first, rest, v ) =
                            split pair

                        entry =
                            ( String.join "." rest, v )
                    in
                    case acc of
                        ( k, entries ) :: more ->
                            if k == first then
                                ( k, entry :: entries ) :: more

                            else
                                ( first, [ entry ] ) :: acc

                        [] ->
                            [ ( first, [ entry ] ) ]
                )
                []
                pairs
    in
    E.object
        (List.map
            (\( k, entries ) ->
                case entries of
                    [ ( "", v ) ] ->
                        ( k, v )

                    _ ->
                        ( k, nest entries )
            )
            grouped
        )
