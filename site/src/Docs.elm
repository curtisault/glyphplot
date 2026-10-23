module Docs exposing (view)

{-| The doc pages (REQ-SITE-030): getting started, the data contract, view
and events, theming, accessibility, performance and the CLI, each with live
charts. They describe glyphplot as built (spec/SPEC.md); what is planned and
not built is said to be planned.
-}

import Data
import Dict
import Html exposing (Html)
import Html.Attributes as A
import Html.Events as Events
import Html.Keyed as Keyed
import Json.Encode as E
import Manifest
import Plot
import Route
import Types exposing (Model, Msg(..))
import Ui


view : Model -> String -> Maybe (List (Html Msg))
view model page =
    let
        body =
            case page of
                "getting-started" ->
                    Just (gettingStarted model)

                "data" ->
                    Just (dataContract model)

                "view-and-events" ->
                    Just (viewAndEvents model)

                "theming" ->
                    Just (theming model)

                "accessibility" ->
                    Just (accessibility model)

                "performance" ->
                    Just (performance model)

                "cli" ->
                    Just (cli model)

                _ ->
                    Nothing
    in
    Maybe.map (\b -> b ++ [ nextPage model page ]) body


nextPage : Model -> String -> Html Msg
nextPage model page =
    let
        after =
            Route.docPages
                |> List.foldl
                    (\( slug, title ) ( found, next ) ->
                        if found && next == Nothing then
                            ( found, Just ( slug, title ) )

                        else
                            ( found || slug == page, next )
                    )
                    ( False, Nothing )
                |> Tuple.second
    in
    case after of
        Just ( slug, title ) ->
            Html.p [ A.class "next" ] [ Html.text "Next: ", Html.a [ Ui.href model (Route.Doc slug) ] [ Html.text title ] ]

        Nothing ->
            Html.p [ A.class "next" ] [ Html.text "Next: ", Html.a [ Ui.href model (Route.Gallery Route.noFilters) ] [ Html.text "the examples" ] ]



-- CHARTS


{-| A live chart in a doc page, keyed by its id so each stays one element.
-}
chart : String -> List ( String, String ) -> Html Msg
chart id attrs =
    Keyed.node "div"
        [ A.class "stage" ]
        [ ( id, Plot.glyphPlot (A.id id :: List.map (\( k, v ) -> A.attribute k v) attrs) ) ]


{-| A live chart whose events go to the page's log.
-}
loggedChart : String -> List ( String, String ) -> Html Msg
loggedChart id attrs =
    Keyed.node "div"
        [ A.class "stage" ]
        [ ( id, Plot.glyphPlot (A.id id :: List.map (\( k, v ) -> A.attribute k v) attrs ++ Plot.onEvents (PlotEvent id)) ) ]


figure : String -> Html Msg -> Html Msg
figure caption content =
    Html.figure [ A.class "chart-figure" ] [ content, Html.figcaption [] (Ui.inline caption) ]


h2 : String -> Html msg
h2 t =
    Html.h2 [] [ Html.text t ]


lineSet : Data.Dataset
lineSet =
    Data.series 11 240


shortSet : Data.Dataset
shortSet =
    Data.series 5 90


candleSet : Data.Dataset
candleSet =
    Data.candles 2026 120


sparkSet : Data.Dataset
sparkSet =
    Data.spark 73 90


million : String
million =
    "1,000,000"


reply : Model -> String -> String -> Maybe String
reply model id method =
    Dict.get (id ++ "/" ++ method) model.replies



-- GETTING STARTED


gettingStarted : Model -> List (Html Msg)
gettingStarted model =
    let
        snippet =
            String.join "\n"
                [ "<glyph-plot type=\"line\" label=\"Hello, line\""
                , "  data='" ++ Data.hello.json ++ "'></glyph-plot>"
                , ""
                , "<script type=\"module\">"
                , "  import \"glyphplot/element\";"
                , "</script>"
                ]
    in
    [ Html.h1 [] [ Html.text "Getting started" ]
    , Html.p [ A.class "lede" ] (Ui.inline "glyphplot draws charts in Unicode glyphs, one code point and one colour a cell. A page shows one with one tag, `<glyph-plot>`, which loads the WebAssembly module of that chart's family when the chart comes near the screen.")
    , h2 "Install"
    , Ui.install model "install-start"
    , h2 "A first chart"
    , Ui.codeBlock model "hello" "index.html" snippet
    , figure "The snippet above, live: eight points in the columns shorthand." (chart "doc-hello" [ ( "type", "line" ), ( "label", "Hello, line" ), ( "rows", "12" ), ( "data", Data.hello.json ) ])
    , Ui.para "Importing `glyphplot/element` defines `<glyph-plot>` once and adds one `<style>` holding the bundled font and the element's rules, each wrapped in `:where()` so any rule of the page wins. Where there is no DOM it does nothing."
    , Ui.para "The `data` attribute takes JSON text: here an object of arrays, a column each. Setting the `data` property from script takes the same forms and typed arrays (the data contract page). Hover the chart for a readout; drag to pan; hold Ctrl or ⌘ and scroll to zoom; double-click to reset; or press Tab and use the arrow keys."
    , h2 "Its size"
    , Ui.para "The element is a block as wide as its container. Without a height of its own it is `rows` rows tall, 12 when `rows` is not set. A cell is `round(0.6 × font-size × devicePixelRatio)` device pixels wide and twice that tall; a font smaller than 11 CSS px (9 at a ratio of 2 or more) is raised to that floor. The grid is the whole cells that fit, the remainder split evenly as padding, and it is laid out again within a frame when the box changes."
    , Ui.para "With both `cols` and `rows` the grid is that size; `fit=\"width\"` scales it to the content width and `fit=\"contain\"` to the largest box that keeps the cell's shape."
    , h2 "Loading"
    , Ui.para "A chart fetches nothing until it comes within one screen height of the viewport (`loading=\"eager\"` loads at once). It then loads its family's module, once a page however many charts use it. This build's modules, and the gzipped size of each (gzip -9 of the built wasm, written to web/pkg/sizes.json by `mise run build:wasm`):"
    , modulesTable model
    , h2 "A tag of its own"
    , Ui.codeBlock model
        "define"
        "JavaScript"
        (String.join "\n"
            [ "import { define } from \"glyphplot/element\";"
            , ""
            , "// <candle-chart> draws candles whatever its type attribute says."
            , "define(\"candle-chart\", { type: \"candles\" });"
            ]
        )
    ]


modulesTable : Model -> Html Msg
modulesTable model =
    Ui.table [ "Chart type", "Module", "Gzipped", "Budget" ]
        (List.map
            (\m ->
                [ Html.a [ Ui.href model (Route.ChartType m.slug) ] [ Html.code [] [ Html.text m.slug ] ]
                , Html.code [] [ Html.text m.module_ ]
                , Html.text (Dict.get m.module_ model.site.sizes |> Maybe.map bytes |> Maybe.withDefault "not measured")
                , Html.text (bytes m.budgetGzip)
                ]
            )
            model.site.charts
        )


bytes : Int -> String
bytes n =
    let
        s =
            String.fromInt n

        groups str =
            if String.length str <= 3 then
                [ str ]

            else
                groups (String.dropRight 3 str) ++ [ String.right 3 str ]
    in
    String.join "," (groups s) ++ " B"



-- THE DATA CONTRACT


dataContract : Model -> List (Html Msg)
dataContract model =
    let
        rows =
            "[{\"region\":\"North\",\"units\":12},{\"region\":\"South\",\"units\":7},{\"region\":\"East\",\"units\":15},{\"region\":\"West\",\"units\":9}]"

        walkCsv =
            Route.toPath model.site.base Route.Home ++ "data/walk.csv"
    in
    [ Html.h1 [] [ Html.text "The data contract" ]
    , Html.p [ A.class "lede" ] [ Html.text "Every chart draws data the page gives it: named tables of typed columns. The element takes that full form and two shorthands, and the shell turns each into typed columns before anything reaches the chart's module." ]
    , h2 "What each chart takes"
    , Ui.table [ "Chart type", "Data", "Modes" ]
        (List.map
            (\m -> [ Html.code [] [ Html.text m.slug ], Html.text (String.join " or " m.shapes), Html.text (String.join ", " m.modes) ])
            model.site.charts
        )
    , h2 "Three forms"
    , Ui.codeBlock model
        "forms"
        "JavaScript"
        (String.join "\n"
            [ "// The columns shorthand: an array a column."
            , "el.data = { t: [\"2025-01-01T00:00:00Z\", \"2025-01-02T00:00:00Z\"], close: [101.5, 102.25] };"
            , ""
            , "// The rows shorthand: an object a row."
            , "el.data = [{ t: \"2025-01-01T00:00:00Z\", close: 101.5 }, { t: \"2025-01-02T00:00:00Z\", close: 102.25 }];"
            , ""
            , "// The data object: named tables of typed columns."
            , "el.data = {"
            , "  version: 1,"
            , "  tables: {"
            , "    prices: {"
            , "      columns: ["
            , "        { name: \"t\", type: \"timestamp\", values: [1735689600000, 1735776000000] },"
            , "        { name: \"close\", type: \"f64\", values: [101.5, 102.25] },"
            , "      ],"
            , "    },"
            , "  },"
            , "};"
            ]
        )
    , Ui.para "In the shorthands the shell infers each column's type: numbers are `f64`, booleans `bool`, `Date` objects and RFC 3339 strings `timestamp` (epoch milliseconds, UTC), other strings `category`, and a typed array `f64`. The full form also has `f32`, `i64`, `u32`, `duration` and `string` columns, and matrices; spec/schemas/data.schema.json is the whole of it."
    , figure "The rows shorthand as the `data` attribute: a category column and a value column drawn as bars."
        (chart "doc-rows" [ ( "type", "bar" ), ( "rows", "12" ), ( "label", "Units by region" ), ( "data", rows ) ])
    , Ui.codeBlock model "rows-attr" "HTML" ("<glyph-plot type=\"bar\" rows=\"12\" data='" ++ rows ++ "'></glyph-plot>")
    , h2 "Missing values"
    , Ui.para "JSON `null`, an empty CSV field, `NaN` and a cleared Arrow validity bit all read as missing. A missing y breaks a line into separate runs; a row with a missing x is not drawn. The option `spanGaps: true` joins the line across a missing y instead."
    , Html.div [ A.class "pair" ]
        [ figure "Three missing values break the line." (chart "doc-gap" [ ( "type", "line" ), ( "rows", "10" ), ( "label", "A series with gaps" ), ( "data", Data.gaps.json ) ])
        , figure "The same data with `spanGaps: true`." (chart "doc-span" [ ( "type", "line" ), ( "rows", "10" ), ( "label", "The same series, gaps spanned" ), ( "options", "{\"spanGaps\":true}" ), ( "data", Data.gaps.json ) ])
        ]
    , h2 "Which column is which"
    , Ui.para "A chart finds each channel (x, y, open, high, …) by the option `encode` first, then by a column's `role`, then by a column of the channel's name. Without any of these the line chart takes the first timestamp column, else the first numeric column, as x and the rest as series, so `{\"t\": […], \"close\": […]}` draws with no options. A channel it cannot find draws an empty frame and warns once naming it."
    , Ui.codeBlock model "encode" "HTML" "<glyph-plot type=\"line\" options='{\"encode\":{\"x\":\"day\",\"y\":[\"north\",\"south\"]}}'></glyph-plot>"
    , Ui.para "The x column should not decrease. When it does, the chart draws a stably sorted copy, the same frame as sorted data, and warns once."
    , h2 "From a URL"
    , Ui.para "`src` takes the URL of a CSV, JSON, NDJSON or Arrow IPC stream, fetched once a change and parsed in the shell; its type comes from the `Content-Type` or the extension. A failed fetch fires `plot-error` and keeps the data there was."
    , figure ("`src=\"" ++ walkCsv ++ "\"`: a 50-row CSV this site's build writes.") (chart "doc-src" [ ( "type", "line" ), ( "rows", "10" ), ( "label", "A walk read from CSV" ), ( "src", walkCsv ) ])
    , h2 "Streaming"
    , Ui.para "`el.append(rows)` takes one row object, an array of them, or columns of typed arrays, and the shell writes every append made within one animation frame to the chart at once. A row whose x equals the last row's replaces it; a row whose x is less is dropped and counted as late. Each commit of data fires `plot-data` with the rows held, the rows dropped, the late rows and the data version."
    , Ui.para "A streaming chart keeps its rows in a ring of `retain.rows` rows (1,000,000 by default) that never reallocates once full; `retain.age` drops rows older than the newest x minus a duration (`\"15min\"`). `paused` keeps appended rows without drawing them."
    , Ui.codeBlock model
        "append"
        "JavaScript"
        (String.join "\n"
            [ "el.options = { retain: { rows: 10000, age: \"15min\" } };"
            , "el.addEventListener(\"plot-data\", (e) => console.log(e.detail)); // { rows, dropped, late, version }"
            , "setInterval(() => el.append({ t: new Date(), load: Math.random() }), 250);"
            ]
        )
    ]



-- VIEW AND EVENTS


viewAndEvents : Model -> List (Html Msg)
viewAndEvents model =
    let
        id =
            "doc-view"

        log =
            List.filter (\e -> String.startsWith "doc-view" e.plot) model.log

        march =
            E.object [ ( "x", E.object [ ( "domain", E.list E.int [ 1740787200000, 1743465600000 ] ) ] ) ]
    in
    [ Html.h1 [] [ Html.text "View and events" ]
    , Html.p [ A.class "lede" ] (Ui.inline "A chart's interactive state is one object, its view: what the reader panned and zoomed to, the cursor, the brush and the series hidden. The chart's frame is a function of its data, options, view and play time alone.")
    , loggedChart id [ ( "type", "line" ), ( "rows", "16" ), ( "label", "A line chart whose events are logged below" ), ( "data", lineSet.json ) ]
    , Html.p [ A.class "muted" ] [ Ui.simulated lineSet.seed ]
    , Html.div [ A.class "buttons" ]
        [ Html.button [ A.type_ "button", Events.onClick (CallPlot id "setView" march) ] [ Html.text "Show March 2025" ]
        , Html.button [ A.type_ "button", Events.onClick (CallPlot id "resetView" E.null) ] [ Html.text "Reset view" ]
        ]
    , Html.div [ A.class "panels" ]
        [ Html.section [ A.class "log", A.attribute "aria-label" "The view" ]
            [ Html.h3 [] [ Html.text "el.view after the last plot-view" ]
            , Html.pre [ A.class "view-json", A.tabindex 0 ] [ Html.text (Dict.get id model.views |> Maybe.withDefault "Pan or zoom the chart.") ]
            ]
        , Plot.logView { title = "Events", clear = ClearLog } log
        ]
    , h2 "The view"
    , Ui.para "`el.view` gets the whole view and sets a whole or partial one, merged key by key; a key set to `null` returns to its default, and `el.resetView()` returns the whole view to the fitted default. It has `x` (`domain`, `follow`), `y` (`domain`, `fit`), `axes` for further y scales, `cursor`, `selection`, `hidden`, `time` and `motion`. Every domain is in data units (epoch milliseconds for time) and `null` means fitted to the data. The `view` attribute takes the same as JSON text."
    , Ui.para "Setting `view`, `data`, `options` or `time` from script fires none of the reader's events, so a page can set one chart's view from another's `plot-view` without a loop. Setting `JSON.parse(JSON.stringify(el.view))` back draws the same frame, so a view can live in a URL."
    , Ui.codeBlock model
        "view-js"
        "JavaScript"
        (String.join "\n"
            [ "// What the \"Show March 2025\" button does:"
            , "el.view = { x: { domain: [Date.UTC(2025, 2, 1), Date.UTC(2025, 3, 1)] } };"
            , ""
            , "// Follow a stream's newest row, keeping the span:"
            , "el.view = { x: { follow: true } };"
            ]
        )
    , h2 "The events"
    , Ui.para "Each is a `CustomEvent` that bubbles and is composed. The reader's events carry `source` (`pointer`, `touch`, `keyboard` or `wheel`); spec/schemas/events.schema.json gives every field."
    , Ui.table [ "Event", "When", "detail" ]
        (List.map (\( n, w, d ) -> [ Html.code [] [ Html.text n ], Html.span [] (Ui.inline w), Html.span [] (Ui.inline d) ])
            [ ( "plot-ready", "the first frame of a chart", "`type`, `cols`, `rows`" )
            , ( "plot-error", "loading or the first frame failed", "`type`, `message`" )
            , ( "plot-hover", "the datum under the cursor changed, at most once a frame", "`series`, `index`, `x`, `y`, `datum`, `aggregate`, `cell`; `null` when the cursor leaves" )
            , ( "plot-select", "a click, tap or Enter on a datum", "`series`, `index`, `x`, `y`, `datum`, `selection`" )
            , ( "plot-view", "a pan, zoom, reset or follow: at most once a frame during a gesture, then once with `final: true`", "`cause`, `final`, `view`" )
            , ( "plot-cursor", "the cursor was locked, unlocked or stepped", "`x`, `locked`" )
            , ( "plot-brush", "a range was brushed or cleared", "`x`, `y`, `count`, `final`" )
            , ( "plot-legend", "a series was shown or hidden", "`series`, `visible`, `hidden`" )
            , ( "plot-data", "a commit of data took effect", "`rows`, `dropped`, `late`, `version`" )
            ]
        )
    , h2 "The reader's input"
    , Ui.para "By default a mouse drag pans, Shift and a drag brushes, Ctrl or ⌘ with the wheel and a trackpad pinch zoom about the pointer, the plain wheel scrolls the page, and a double-click resets. The options `drag` (`pan`, `brush`, `zoom`) and `wheel` (`scroll`, `zoom`) change the drag and the wheel. On touch one finger scrubs the cursor, two pan, a pinch zooms, a double-tap resets, a long press brushes, and a vertical swipe scrolls the page. The keys are on the accessibility page."
    , h2 "Linked charts"
    , Ui.para "Charts with the same `link` value share their x domain and their cursor's x, in data units, so charts of different widths agree. Pan either one below."
    , Html.div [ A.class "pair uneven" ]
        [ chart "doc-link-a" [ ( "type", "line" ), ( "rows", "10" ), ( "link", "doc-pair" ), ( "label", "Linked chart, wide" ), ( "data", shortSet.json ) ]
        , chart "doc-link-b" [ ( "type", "candles" ), ( "rows", "10" ), ( "link", "doc-pair" ), ( "label", "Linked chart, narrow" ), ( "options", "{\"xScale\":\"time\",\"volume\":false}" ), ( "data", candleSet.json ) ]
        ]
    , Html.p [ A.class "muted" ] [ Ui.simulated shortSet.seed, Html.text " ", Ui.simulated candleSet.seed ]
    ]



-- THEMING


theming : Model -> List (Html Msg)
theming model =
    let
        themed mode =
            figure ("`theme=\"" ++ mode ++ "\"`")
                (chart ("doc-theme-" ++ mode) [ ( "type", "line" ), ( "rows", "9" ), ( "theme", mode ), ( "label", "Theme " ++ mode ), ( "data", shortSet.json ) ])

        current =
            Manifest.themeToString (Types.effectiveTheme model)
    in
    [ Html.h1 [] [ Html.text "Theming" ]
    , Html.p [ A.class "lede" ] (Ui.inline ("A chart takes its colours from CSS custom properties inherited from the page, so a page's own theme rules re-theme every chart. This page is in its " ++ current ++ " theme now: the theme button at the top sets `data-theme` on `<html>`, and every chart here follows."))
    , Html.div [ A.class "grid4" ] (List.map themed [ "auto", "dark", "light", "paper" ])
    , Html.p [ A.class "muted" ] [ Ui.simulated shortSet.seed ]
    , h2 "The tokens"
    , Ui.para "The element reads `--glyphplot-ground`, `--glyphplot-ink`, `--glyphplot-muted`, `--glyphplot-rule`, `--glyphplot-accent`, `--glyphplot-series-1` to `--glyphplot-series-8`, `--glyphplot-up`, `--glyphplot-down` and `--glyphplot-scale` from its computed style. A token the page does not set falls back to the default set. Without a ground the chart draws none, over the page's background, and picks the light defaults when the computed text colour is dark (luma under 128) and the dark ones otherwise."
    , Ui.codeBlock model
        "css"
        "CSS"
        (String.join "\n"
            [ ":root { --glyphplot-series-1: #b45309; --glyphplot-series-2: #0e7490; }"
            , "[data-theme=\"dark\"] { --glyphplot-series-1: #fbbf24; --glyphplot-series-2: #67e8f9; }"
            ]
        )
    , figure "Series 1 and 2 set by custom properties in this site's stylesheet, a pair for each theme, on a box around the chart."
        (Html.div [ A.class "custom-tokens" ]
            [ chart "doc-tokens" [ ( "type", "line" ), ( "rows", "9" ), ( "label", "Custom series colours" ), ( "data", shortSet.json ) ] ]
        )
    , Ui.para "The element re-resolves its theme and repaints, without a new chart or a new frame, when `prefers-color-scheme` changes, when `<html>`'s `class`, `style` or `data-theme` changes, and when `theme` is set."
    , h2 "The theme attribute and property"
    , Ui.para "`theme` is `auto` (the default: the custom properties and the text colour decide), `dark`, `light` or `paper` (dark on light for print). The `theme` property also takes a theme object of the same tokens, with `series` as an array of up to eight colours, which wins over the custom properties."
    , Ui.codeBlock model "theme-object" "JavaScript" "el.theme = { ground: \"#fffdf5\", ink: \"#1c1917\", series: [\"#7c3aed\", \"#059669\"] };"
    , h2 "One ink"
    , Ui.para "With `mono` a chart draws in one colour into a `<pre>`: line series by dash patterns counted from the x origin, so they do not crawl on a pan, then by markers; bars and areas by `█ ▓ ▒ ░` fills; candles hollow when rising and solid when falling. The option `patterns: \"on\"` adds the patterns in colour too."
    , Html.div [ A.class "pair" ]
        [ figure "`mono`: three series told apart by dashes." (chart "doc-mono" [ ( "type", "line" ), ( "rows", "10" ), ( "mono", "" ), ( "label", "One ink line" ), ( "data", shortSet.json ) ])
        , figure "`mono` candles: rising hollow, falling solid." (chart "doc-mono-candles" [ ( "type", "candles" ), ( "rows", "10" ), ( "mono", "" ), ( "label", "One ink candles" ), ( "options", "{\"volume\":false}" ), ( "data", candleSet.json ) ])
        ]
    , h2 "The defaults"
    , Ui.para "Every default series, up, down and accent colour has a contrast of at least 3:1 against its default ground, and ink and muted at least 4.5:1 (WCAG 2.2). Each default palette's eight colours are at least 15 CIEDE2000 apart under normal vision and under simulated deuteranopia, protanopia and tritanopia. Up and down are a blue and an orange, tokens like the rest, and candles and bars tell them apart by fill as well as colour."
    ]



-- ACCESSIBILITY


accessibility : Model -> List (Html Msg)
accessibility model =
    [ Html.h1 [] [ Html.text "Accessibility" ]
    , Html.p [ A.class "lede" ] [ Html.text "Each chart is a figure with a summary, one tab stop that steps through the data, and a hidden table of what is in view." ]
    , figure "Press Tab to reach the chart, then the arrow keys." (chart "doc-a11y" [ ( "type", "candles" ), ( "rows", "16" ), ( "label", "Simulated daily candles" ), ( "data", candleSet.json ) ])
    , Html.p [ A.class "muted" ] [ Ui.simulated candleSet.seed ]
    , h2 "What a screen reader meets"
    , Html.ul []
        [ Html.li [] (Ui.inline "The element is `role=\"figure\"`, named by `label`, else by its type and series names.")
        , Html.li [] (Ui.inline "Its drawing is `role=\"img\"` with a summary: the chart type, the series, each axis's title and the domain in view, rewritten at most once a second.")
        , Html.li [] (Ui.inline "A `role=\"slider\"` cursor is the chart's one tab stop. Its `aria-valuetext` reads the cursor's x and each series' value there, up to four and then how many more; its `aria-keyshortcuts` lists the keys.")
        , Html.li [] (Ui.inline "A visually hidden `<table>` holds the data in view, at most 500 rows (downsampled, each bucket's minimum and maximum kept), with a caption, column headers and a row header a row.")
        , Html.li [] (Ui.inline "With `announce=\"10\"` (seconds, at least 5) a polite live region reads a stream's latest value, silent while paused or while the cursor has focus. Without it there is no live region.")
        ]
    , h2 "The keys"
    , Ui.keysTable
    , h2 "Without dragging"
    , Ui.para "On focus and on hover a row of buttons (zoom out, zoom in, reset, step back, step on) gives a click for every drag, and a brush can be made with two clicks. The focused chart shows a 2 px outline in the accent colour."
    , h2 "Motion and streams"
    , Ui.para "Under `prefers-reduced-motion: reduce` a view change jumps with no animation, unless the element has `motion`. A stream can be held with `paused` or with Space on the cursor; while paused the chart keeps what arrives and shows the live view on resume. The hidden table of a streaming chart is rebuilt at most once a second, and never while focus is inside it."
    , h2 "Checked"
    , Ui.para "The browser tests scan a page holding every chart with axe-core in light and dark and find no violation of impact serious or critical."
    ]



-- PERFORMANCE


performance : Model -> List (Html Msg)
performance model =
    let
        id =
            "doc-million"

        fillArgs =
            E.object [ ( "n", E.int 1000000 ), ( "seed", E.int 9 ) ]
    in
    [ Html.h1 [] [ Html.text "Performance" ]
    , Html.p [ A.class "lede" ] [ Html.text "A page loads only the modules of the chart families it shows, a chart draws only the cells that changed, and a line draws from four values per dot column however many rows are in view." ]
    , figure ("Press the button to set " ++ million ++ " rows as two `Float64Array` columns, then pan and zoom.")
        (chart id [ ( "type", "line" ), ( "rows", "16" ), ( "label", "A million points" ), ( "data", "{\"x\":[0,1],\"walk\":[0,0]}" ) ])
    , Html.div [ A.class "buttons" ]
        [ Html.button [ A.type_ "button", Events.onClick (CallPlot id "fill" fillArgs) ] [ Html.text ("Set " ++ million ++ " rows") ]
        , Html.button [ A.type_ "button", Events.onClick (CallPlot id "resetView" E.null) ] [ Html.text "Reset view" ]
        ]
    , Html.p [ A.attribute "role" "status", A.class "muted" ]
        [ case reply model id "fill" of
            Just r ->
                Html.text r

            Nothing ->
                Html.span [] (Ui.inline "Not yet set. The button calls a port: Elm cannot make typed arrays, so site/src/main.ts sets `el.data` itself.")
        ]
    , Html.p [ A.class "muted" ] [ Ui.simulated 9 ]
    , h2 "Size"
    , Ui.para "Each chart family is one wasm module, held to a budget of 358,400 bytes gzipped (gzip -9), which `mise run size` checks. The sizes of this build:"
    , modulesTable model
    , h2 "Loading"
    , Ui.para "No module or glue is fetched until a chart comes within one screen height of the viewport, and a module is fetched once a page however many charts use it. The manifest ships in the shell, and the wasm is fetched alongside its glue. Measured on 2026-10-20 with `mise run bench:load` (median of 7 cold loads in headless Chromium), a one-chart page drew its first frame in 796 ms on a 170 ms, 9 Mbps profile, against a budget of 1.0 s, and in 3,022 ms on Lighthouse 12's slow 4G profile."
    , h2 "Frames"
    , Ui.para "A line-like chart draws from M4 aggregates (the first, last, least and greatest value per dot column) whenever the rows in view outnumber four times its dot columns, and that frame equals, cell for cell, the frame drawn from every segment. It finds the rows in view by binary search and caches the aggregates by data version, x range and width. A frame redraws only the cells whose glyph or colours changed, and a hover redraws only the crosshair and readout over a cached base."
    , Ui.table [ "Measured (2026-10-20, line, 1,000,000 points)", "How", "Result" ]
        [ [ Html.text "A frame at 320 × 120", Html.text "natively, tests/budgets.rs, contract profile", Html.text "1.80 ms" ]
        , [ Html.text "A pointer move", Html.text "natively, the same", Html.text "0.010 ms" ]
        , [ Html.text "A frame", Html.text "headless Chromium 156, mise run bench:web", Html.text "4.14 ms" ]
        , [ Html.text "A pointer move", Html.text "headless Chromium 156, the same", Html.text "0.055 ms" ]
        ]
    , Ui.para "Every chart is held to an average of 4 ms a frame with none over 16 ms panning across 1,000,000 points at 80 × 24 and 320 × 120 natively, 8 ms in headless Chromium, and 0.5 ms a pointer move; `mise run bench` and `mise run bench:web` check them."
    , h2 "Streams and memory"
    , Ui.para "Appends made within one frame reach the chart as one write. A ring of `retain.rows` rows grows to its capacity and never reallocates: in Node 26.5.1 3,000,000 rows into a ring of 1,000,000 grew wasm memory not at all after the first 1,000,000, and an append cost 0.75 µs a row through the shell. Appending one sample a millisecond to a full ring of 1,000,000 while drawing at 60 Hz is held to 500 ms of CPU a second in headless Chromium."
    ]



-- THE CLI


cli : Model -> List (Html Msg)
cli model =
    let
        id =
            "doc-cli"
    in
    [ Html.h1 [] [ Html.text "The CLI" ]
    , Html.p [ A.class "lede notice" ] (Ui.inline "The CLI does not draw yet: it draws from v0.8.0. Until then the `glyphplot` binary prints its usage and version, and nothing is on crates.io.")
    , h2 "What is built"
    , Ui.para "The crate `glyphplot-cli` builds the binary `glyphplot`. `glyphplot --help` (or no arguments) prints its usage and the chart families; `glyphplot --version` prints its version; anything else prints the usage to stderr and exits 2."
    , Ui.codeBlock model
        "cli-build"
        "Build and run it from the repository (shell)"
        (String.join "\n"
            [ "cargo run -q -p glyphplot-cli -- --help"
            , "cargo run -q -p glyphplot-cli -- --version"
            ]
        )
    , h2 "What v0.8.0 plans"
    , Ui.para "The plan gives the CLI the same charts in the terminal, from a file or stdin, with keys and the mouse; a `render` command writing text, ANSI, SVG or PNG; `demo` on generated data; and `list`. None of it is built. As planned (docs/IMPLEMENTATION_PLAN.md, \"The CLI\"):"
    , Ui.codeBlock model
        "cli-planned"
        "Planned for v0.8.0, not built (shell)"
        (String.join "\n"
            [ "glyphplot candles --from feed.csv"
            , "tail -f metrics.csv | glyphplot line"
            , "glyphplot render line --from data.csv --format svg -o chart.svg"
            ]
        )
    , h2 "A frame as text, today"
    , Ui.para "In the browser, `el.text()` already gives a chart's frame as one-ink text, one line a row. The chart below is `mono`; the button shows its frame."
    , chart id [ ( "type", "sparkline" ), ( "rows", "4" ), ( "mono", "" ), ( "label", "Two sparklines in one ink" ), ( "data", sparkSet.json ) ]
    , Html.p [ A.class "muted" ] [ Ui.simulated sparkSet.seed ]
    , Html.div [ A.class "buttons" ] [ Html.button [ A.type_ "button", Events.onClick (CallPlot id "text" E.null) ] [ Html.text "Show el.text()" ] ]
    , case reply model id "text" of
        Just t ->
            Html.pre [ A.class "frame-text", A.tabindex 0, A.attribute "aria-label" "The frame as text" ] [ Html.text t ]

        Nothing ->
            Html.text ""
    ]
