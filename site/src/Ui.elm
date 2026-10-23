module Ui exposing
    ( attributeReference
    , codeBlock
    , href
    , inline
    , install
    , keysTable
    , para
    , simulated
    , table
    )

{-| Pieces every page uses: text with `code`, copyable code blocks, the
"simulated" label, the install snippets and the element's reference tables.
-}

import Html exposing (Html)
import Html.Attributes as A
import Html.Events as Events
import Route exposing (Route)
import Types exposing (Model, Msg(..))


{-| Text where `backticks` mark code.
-}
inline : String -> List (Html msg)
inline str =
    String.split "`" str
        |> List.indexedMap
            (\i part ->
                if modBy 2 i == 1 then
                    Html.code [] [ Html.text part ]

                else
                    Html.text part
            )


para : String -> Html msg
para str =
    Html.p [] (inline str)


href : Model -> Route -> Html.Attribute msg
href model route =
    A.href (Route.toPath model.site.base route)


{-| A code block with a copy button; `id` tells copies apart.
-}
codeBlock : Model -> String -> String -> String -> Html Msg
codeBlock model id caption code =
    Html.figure [ A.class "code" ]
        [ Html.figcaption []
            [ Html.span [] [ Html.text caption ]
            , Html.button
                [ A.type_ "button"
                , A.class "copy"
                , Events.onClick (Copy id code)
                , A.attribute "aria-label" ("Copy " ++ caption)
                ]
                [ Html.text
                    (if model.copied == Just id then
                        "Copied"

                     else
                        "Copy"
                    )
                ]
            ]
        , Html.pre [ A.tabindex 0 ] [ Html.code [] [ Html.text code ] ]
        ]


{-| The label generated data carries, with its seed (REQ-SITE-110).
-}
simulated : Int -> Html msg
simulated seed =
    Html.span [ A.class "simulated" ] [ Html.text ("simulated · seed " ++ String.fromInt seed) ]


table : List String -> List (List (Html msg)) -> Html msg
table heads rows =
    Html.div [ A.class "table-wrap" ]
        [ Html.table []
            [ Html.thead [] [ Html.tr [] (List.map (\h -> Html.th [ A.scope "col" ] [ Html.text h ]) heads) ]
            , Html.tbody []
                (List.map
                    (\cells ->
                        case cells of
                            first :: rest ->
                                Html.tr [] (Html.th [ A.scope "row" ] [ first ] :: List.map (\c -> Html.td [] [ c ]) rest)

                            [] ->
                                Html.tr [] []
                    )
                    rows
                )
            ]
        ]


{-| How to install today (REQ-PKG-060): nothing is published, so every snippet says so and builds from the repository.
-}
install : Model -> String -> Html Msg
install model id =
    Html.div [ A.class "install" ]
        [ Html.p [ A.class "notice" ]
            (inline "Not published yet: glyphplot is on neither npm nor crates.io, so `npm install glyphplot` does not work. Build it from the repository and install the built `web/` folder by path.")
        , codeBlock model
            (id ++ "-build")
            "Build from the repository (shell)"
            (String.join "\n"
                [ "git clone https://github.com/curtisault/glyphplot.git"
                , "cd glyphplot"
                , "mise install            # the pinned toolchain: Rust, Node, wasm-pack, …"
                , "mise run install        # the npm packages"
                , "mise run build:wasm     # one wasm module a chart family into web/pkg"
                , "npm run build --prefix web   # the TypeScript shell into web/dist"
                ]
            )
        , codeBlock model
            (id ++ "-npm")
            "Install the built folder in your project (shell)"
            (String.join "\n"
                [ "# in a project with a bundler (Vite, for one), beside the clone"
                , "npm install ../glyphplot/web"
                ]
            )
        ]


{-| The element's attributes as built (the plan's element table, REQ-EL-020).
-}
attributeReference : Html msg
attributeReference =
    table [ "Attribute", "Value", "On change" ]
        (List.map
            (\( a, v, c ) -> [ Html.code [] [ Html.text a ], Html.span [] (inline v), Html.span [] (inline c) ])
            [ ( "type", "a chart type: `line`, `sparkline`, `candles`, `bar`", "a new chart; data and view carried when its meta takes the data's shape" )
            , ( "data", "JSON text: the data object, the columns or the rows shorthand", "replaces the data" )
            , ( "src", "URL of CSV, JSON, NDJSON or Arrow", "fetched, replaces the data" )
            , ( "options", "JSON object text", "new options; data and view kept" )
            , ( "view", "JSON object text", "sets the view; fires no event" )
            , ( "theme", "`auto` (default), `dark`, `light`, `paper`", "repaints" )
            , ( "locale", "a BCP 47 tag", "a new locale pack, repaint" )
            , ( "timeZone", "`UTC`, a fixed offset or an IANA name", "ticks and labels in that zone" )
            , ( "mono", "present or not", "one ink, drawn in a `<pre>`" )
            , ( "motion", "present or not", "view animations play under reduced motion" )
            , ( "label", "text", "the accessible name" )
            , ( "cols, rows", "integers", "fix the grid's size (`rows` alone sets the height; 12 without a CSS height)" )
            , ( "fit", "`width`, `contain`", "with `cols` and `rows`: scales the grid to the box" )
            , ( "loading", "`lazy` (default), `eager`", "`eager` loads at once instead of near the screen" )
            , ( "link", "a group name", "shares `x.domain` and `cursor.x` with the group" )
            , ( "paused", "present or not", "streamed data is kept, not drawn" )
            , ( "announce", "seconds, at least 5", "a polite live region of the latest value" )
            , ( "time", "seconds", "holds the frame at `t` (tests, snapshots)" )
            ]
        )


{-| The keys on a focused chart (the plan's keys table, REQ-KEY-010).
-}
keysTable : Html msg
keysTable =
    table [ "Key", "Does" ]
        (List.map (\( k, d ) -> [ Html.kbd [] [ Html.text k ], Html.text d ])
            [ ( "← → (↓ ↑)", "the cursor to the previous or next datum; past the edge, the view pans" )
            , ( "Home, End", "the first or last datum in view; End also follows a stream's live end" )
            , ( "PageUp, PageDown", "the cursor a tenth of the view" )
            , ( "Shift + ← →", "pan a tenth of the view" )
            , ( "+ (=), −", "zoom about the cursor" )
            , ( "0", "reset the view" )
            , ( "Enter", "lock the cursor and select the datum" )
            , ( "Shift + Enter", "start, then commit, a brush from the cursor" )
            , ( "Escape", "hide the readout, clear the brush, unlock; a second Escape leaves the chart" )
            , ( "Space", "pause or resume a stream" )
            , ( "1–9", "show or hide the nth series" )
            ]
        )
