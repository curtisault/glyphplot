module Manifest exposing
    ( Example
    , Feed
    , Meta
    , Site
    , ThemeName(..)
    , decoder
    , exampleOf
    , examplesOfType
    , metaOf
    , themeFromString
    , themeToString
    )

{-| The site's flags (REQ-SITE-010): the charts' metas, modules and budgets
from web/pkg/modules.js, each module's gzipped size from web/pkg/sizes.json,
and the gallery from site/examples/manifest.json with each example's seed
from its module. main.ts reads them before the page shows; no wasm is
loaded for them.
-}

import Dict exposing (Dict)
import Json.Decode as D exposing (Decoder)
import Json.Encode exposing (Value)


{-| One chart type's meta (spec/schemas/meta.schema.json), with the module that plays it.
-}
type alias Meta =
    { slug : String
    , name : String
    , note : String
    , family : String
    , modes : List String
    , shapes : List String
    , options : List ( String, Value )
    , minCols : Int
    , minRows : Int
    , module_ : String
    , budgetGzip : Int
    , frameMs : Float
    , worstMs : Float
    , inputMs : Float
    }


{-| A live third-party feed's source and licence (REQ-SITE-050).
-}
type alias Feed =
    { source : String
    , url : String
    , licence : String
    }


{-| One gallery entry (site/examples/api.ts, ManifestEntry), with `seed` from its module (REQ-SITE-110).
-}
type alias Example =
    { slug : String
    , title : String
    , what : String
    , types : List String
    , modes : List String
    , domain : String
    , features : List String
    , datasets : List String
    , milestone : String
    , feed : Maybe Feed
    , seed : Maybe Int
    }


type ThemeName
    = Light
    | Dark


{-| Everything the flags carry.
-}
type alias Site =
    { charts : List Meta
    , sizes : Dict String Int
    , examples : List Example
    , base : String
    , storedTheme : Maybe ThemeName
    , systemTheme : ThemeName
    }


themeFromString : String -> Maybe ThemeName
themeFromString str =
    case str of
        "light" ->
            Just Light

        "dark" ->
            Just Dark

        _ ->
            Nothing


themeToString : ThemeName -> String
themeToString t =
    case t of
        Light ->
            "light"

        Dark ->
            "dark"


decoder : Decoder Site
decoder =
    D.map6 Site
        (D.field "charts" (D.list metaDecoder) |> D.andThen withModules)
        (D.field "sizes" (D.dict D.int))
        (D.field "examples" (D.list exampleDecoder))
        (D.field "base" D.string)
        (D.field "storedTheme" (D.nullable D.string) |> D.map (Maybe.andThen themeFromString))
        (D.field "systemTheme" D.string |> D.map (themeFromString >> Maybe.withDefault Light))


withModules : List Meta -> Decoder (List Meta)
withModules metas =
    D.field "modules" (D.dict D.string)
        |> D.map
            (\modules ->
                List.map
                    (\m -> { m | module_ = Dict.get m.slug modules |> Maybe.withDefault m.family })
                    metas
            )


metaDecoder : Decoder Meta
metaDecoder =
    D.succeed Meta
        |> field "slug" D.string
        |> field "name" D.string
        |> field "note" D.string
        |> field "family" D.string
        |> field "modes" (D.list D.string)
        |> field "shapes" (D.list D.string)
        |> field "options" (D.oneOf [ D.keyValuePairs D.value, D.null [] ])
        |> field "min" (D.field "cols" D.int)
        |> field "min" (D.field "rows" D.int)
        |> andMap (D.succeed "")
        |> field "budget" (D.field "gzip" D.int)
        |> field "budget" (D.field "frameMs" D.float)
        |> field "budget" (D.field "worstMs" D.float)
        |> field "budget" (D.field "inputMs" D.float)


exampleDecoder : Decoder Example
exampleDecoder =
    D.succeed Example
        |> field "slug" D.string
        |> field "title" D.string
        |> field "what" D.string
        |> optional "types" (D.list D.string) []
        |> optional "modes" (D.list D.string) []
        |> optional "domain" D.string ""
        |> optional "features" (D.list D.string) []
        |> optional "datasets" (D.list D.string) []
        |> optional "milestone" D.string ""
        |> optional "feed" (D.nullable feedDecoder) Nothing
        |> optional "seed" (D.nullable D.int) Nothing


feedDecoder : Decoder Feed
feedDecoder =
    D.map3 Feed
        (D.field "source" D.string)
        (D.field "url" D.string)
        (D.field "licence" D.string)


andMap : Decoder a -> Decoder (a -> b) -> Decoder b
andMap =
    D.map2 (|>)


field : String -> Decoder a -> Decoder (a -> b) -> Decoder b
field name d =
    andMap (D.field name d)


optional : String -> Decoder a -> a -> Decoder (a -> b) -> Decoder b
optional name d fallback =
    andMap (D.oneOf [ D.field name d, D.succeed fallback ])


metaOf : String -> List Meta -> Maybe Meta
metaOf slug =
    List.filter (\m -> m.slug == slug) >> List.head


exampleOf : String -> List Example -> Maybe Example
exampleOf slug =
    List.filter (\e -> e.slug == slug) >> List.head


examplesOfType : String -> List Example -> List Example
examplesOfType t =
    List.filter (\e -> List.member t e.types)
