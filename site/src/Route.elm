module Route exposing
    ( Filters
    , Route(..)
    , docPages
    , filterKeys
    , fromUrl
    , getFilter
    , noFilters
    , setFilter
    , toPath
    )

{-| The site's routes (REQ-SITE-010): `/`, `/<type>`, `/docs/<page>`,
`/examples` (the gallery, its filters in the query) and `/examples/<slug>`.
Every path is under the site's base, `/` unless the build sets another.
-}

import Url exposing (Url)
import Url.Builder as Builder
import Url.Parser as Parser exposing ((</>), (<?>), Parser, s, string, top)
import Url.Parser.Query as Query


type Route
    = Home
    | ChartType String
    | Doc String
    | Gallery Filters
    | Example String
    | NotFound


{-| The gallery's filters: chart type, mode, domain, feature and milestone; Nothing is any.
-}
type alias Filters =
    { type_ : Maybe String
    , mode : Maybe String
    , domain : Maybe String
    , feature : Maybe String
    , milestone : Maybe String
    }


noFilters : Filters
noFilters =
    Filters Nothing Nothing Nothing Nothing Nothing


{-| The query keys, in the order the gallery shows its filters.
-}
filterKeys : List String
filterKeys =
    [ "type", "mode", "domain", "feature", "milestone" ]


getFilter : String -> Filters -> Maybe String
getFilter key f =
    case key of
        "type" ->
            f.type_

        "mode" ->
            f.mode

        "domain" ->
            f.domain

        "feature" ->
            f.feature

        "milestone" ->
            f.milestone

        _ ->
            Nothing


{-| Sets one filter; an empty value clears it.
-}
setFilter : String -> String -> Filters -> Filters
setFilter key value f =
    let
        v =
            if value == "" then
                Nothing

            else
                Just value
    in
    case key of
        "type" ->
            { f | type_ = v }

        "mode" ->
            { f | mode = v }

        "domain" ->
            { f | domain = v }

        "feature" ->
            { f | feature = v }

        "milestone" ->
            { f | milestone = v }

        _ ->
            f


{-| The doc pages, in reading order: slug and title (REQ-SITE-030).
-}
docPages : List ( String, String )
docPages =
    [ ( "getting-started", "Getting started" )
    , ( "data", "The data contract" )
    , ( "view-and-events", "View and events" )
    , ( "theming", "Theming" )
    , ( "accessibility", "Accessibility" )
    , ( "performance", "Performance" )
    , ( "cli", "The CLI" )
    ]


parser : Parser (Route -> a) a
parser =
    Parser.oneOf
        [ Parser.map Home top
        , Parser.map Doc (s "docs" </> string)
        , Parser.map Gallery (s "examples" <?> filtersQuery)
        , Parser.map Example (s "examples" </> string)
        , Parser.map ChartType string
        ]


filtersQuery : Query.Parser Filters
filtersQuery =
    Query.map5 Filters
        (Query.string "type")
        (Query.string "mode")
        (Query.string "domain")
        (Query.string "feature")
        (Query.string "milestone")


{-| The route of a URL under `base` (`/` or `/some/folder/`). A path outside the base is NotFound.
-}
fromUrl : String -> Url -> Route
fromUrl base url =
    let
        prefix =
            normalBase base
    in
    if String.startsWith prefix url.path || url.path ++ "/" == prefix then
        Parser.parse parser { url | path = "/" ++ String.dropLeft (String.length prefix) url.path, fragment = Nothing }
            |> Maybe.withDefault NotFound

    else
        NotFound


{-| The absolute path of a route under `base`.
-}
toPath : String -> Route -> String
toPath base route =
    let
        prefix =
            normalBase base

        under segments query =
            prefix ++ Builder.relative segments query
    in
    case route of
        Home ->
            prefix

        ChartType t ->
            under [ t ] []

        Doc page ->
            under [ "docs", page ] []

        Gallery f ->
            under [ "examples" ]
                (List.filterMap
                    (\key -> getFilter key f |> Maybe.map (Builder.string key))
                    filterKeys
                )

        Example slug ->
            under [ "examples", slug ] []

        NotFound ->
            prefix


{-| `/` for an empty or relative base, else the base with one leading and one trailing slash.
-}
normalBase : String -> String
normalBase base =
    let
        trimmed =
            base
                |> dropWhileStart "."
                |> dropWhileStart "/"
                |> dropWhileEnd "/"
    in
    if trimmed == "" then
        "/"

    else
        "/" ++ trimmed ++ "/"


dropWhileStart : String -> String -> String
dropWhileStart c str =
    if String.startsWith c str then
        dropWhileStart c (String.dropLeft 1 str)

    else
        str


dropWhileEnd : String -> String -> String
dropWhileEnd c str =
    if String.endsWith c str then
        dropWhileEnd c (String.dropRight 1 str)

    else
        str
