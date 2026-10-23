module ChartPage exposing (plotId, view)

{-| A page per chart type (REQ-SITE-020): the chart live on a generated
dataset, a control per option, the element's attributes, a log of its events,
an example from the gallery and usage snippets.
-}

import Data
import Dict
import Html exposing (Html)
import Html.Attributes as A
import Html.Events as Events
import Html.Keyed as Keyed
import Html.Lazy as Lazy
import Json.Encode as E
import Manifest exposing (Meta)
import Options exposing (Control(..), Field)
import Plot
import Route
import Types exposing (Model, Msg(..))
import Ui


plotId : String -> String
plotId t =
    "plot-" ++ t


view : Model -> Meta -> List (Html Msg)
view model meta =
    let
        t =
            meta.slug

        id =
            plotId t

        st =
            model.chart

        fields =
            Options.fields meta.modes meta.options

        optionsJson =
            Options.overridesJson fields st.inputs

        dataset =
            st.dataset

        attrs =
            elementAttributes t optionsJson st dataset

        log =
            List.filter (\e -> e.plot == id) model.log
    in
    [ Html.h1 [] [ Html.code [] [ Html.text t ] ]
    , Html.p [ A.class "lede" ] [ Html.text (capitalise meta.note ++ ".") ]
    , facts model meta
    , Html.section [ A.class "live", A.attribute "aria-labelledby" "live-h" ]
        [ Html.h2 [ A.id "live-h" ] [ Html.text "Live" ]
        , Html.p [ A.class "muted" ]
            [ Ui.simulated dataset.seed
            , Html.text (" " ++ String.fromInt dataset.rows ++ " rows of generated data in the columns shorthand: " ++ String.join ", " dataset.columns ++ ".")
            ]
        , Lazy.lazy6 stage st.dataset t optionsJson st.theme st.mono st.rows
        , Html.div [ A.class "buttons" ]
            [ Html.button [ A.type_ "button", Events.onClick (CallPlot id "resetView" E.null) ] [ Html.text "Reset view" ]
            , Html.button [ A.type_ "button", Events.onClick ResetOptions ] [ Html.text "Default options" ]
            , Html.button [ A.type_ "button", Events.onClick (CallPlot id "text" E.null) ] [ Html.text "Frame as text" ]
            ]
        , case Dict.get (id ++ "/text") model.replies of
            Just text ->
                Html.pre [ A.class "frame-text", A.attribute "aria-label" "The frame as text", A.tabindex 0 ] [ Html.text text ]

            Nothing ->
                Html.text ""
        , Html.div [ A.class "panels" ]
            [ controls meta fields st
            , Plot.logView { title = "Events", clear = ClearLog } log
            ]
        ]
    , Html.section [ A.attribute "aria-labelledby" "attrs-h" ]
        [ Html.h2 [ A.id "attrs-h" ] [ Html.text "Its attributes" ]
        , Ui.para "What the chart above holds now. The controls change them; the element takes each change without a new chart, keeping its data and view, except `type`."
        , Ui.table [ "Attribute", "Value" ]
            (List.map (\( k, v ) -> [ Html.code [] [ Html.text k ], Html.code [] [ Html.text (shown k v dataset) ] ]) attrs)
        ]
    , exampleSection model t
    , Html.section [ A.attribute "aria-labelledby" "use-h" ]
        [ Html.h2 [ A.id "use-h" ] [ Html.text "Use it" ]
        , Ui.install model ("install-" ++ t)
        , Ui.codeBlock model ("html-" ++ t) "On a page, with a bundler (HTML)" (htmlSnippet t optionsJson st dataset)
        , Ui.codeBlock model ("js-" ++ t) "The JS API, no element (JavaScript)" (jsSnippet t optionsJson st)
        ]
    , Html.section [ A.attribute "aria-labelledby" "ref-h" ]
        [ Html.h2 [ A.id "ref-h" ] [ Html.text "Every attribute" ]
        , Ui.attributeReference
        ]
    ]


capitalise : String -> String
capitalise s =
    String.toUpper (String.left 1 s) ++ String.dropLeft 1 s


facts : Model -> Meta -> Html Msg
facts model meta =
    let
        size =
            Dict.get meta.module_ model.site.sizes
    in
    Html.dl [ A.class "facts" ]
        (List.concatMap (\( k, v ) -> [ Html.dt [] [ Html.text k ], Html.dd [] v ])
            [ ( "Modes", [ Html.text (String.join ", " meta.modes) ] )
            , ( "Data", [ Html.text (String.join " or " meta.shapes) ] )
            , ( "Module"
              , [ Html.code [] [ Html.text meta.module_ ]
                , Html.text
                    (case size of
                        Just bytes ->
                            ", " ++ kb bytes ++ " gzipped (gzip -9 of the built wasm, web/pkg/sizes.json), against a budget of " ++ kb meta.budgetGzip

                        Nothing ->
                            ""
                    )
                ]
              )
            , ( "Smallest size with axes", [ Html.text (String.fromInt meta.minCols ++ " × " ++ String.fromInt meta.minRows ++ " cells") ] )
            , ( "Time budget", [ Html.text (String.fromFloat meta.frameMs ++ " ms a frame on average, " ++ String.fromFloat meta.worstMs ++ " ms at worst, " ++ String.fromFloat meta.inputMs ++ " ms a pointer move") ] )
            ]
        )


kb : Int -> String
kb bytes =
    String.fromFloat (toFloat (round (toFloat bytes / 102.4)) / 10) ++ " KiB"


elementAttributes : String -> String -> { a | theme : String, mono : Bool, rows : String } -> Data.Dataset -> List ( String, String )
elementAttributes t optionsJson st dataset =
    List.concat
        [ [ ( "type", t ), ( "rows", st.rows ), ( "label", "A " ++ t ++ " chart of simulated data" ) ]
        , if optionsJson == "{}" then
            []

          else
            [ ( "options", optionsJson ) ]
        , if st.theme == "auto" then
            []

          else
            [ ( "theme", st.theme ) ]
        , if st.mono then
            [ ( "mono", "" ) ]

          else
            []
        , [ ( "data", dataset.json ) ]
        ]


shown : String -> String -> Data.Dataset -> String
shown key value dataset =
    if key == "data" then
        "JSON text, " ++ String.fromInt (String.length value) ++ " characters: " ++ String.fromInt dataset.rows ++ " rows of " ++ String.join ", " dataset.columns

    else if key == "mono" then
        "(present)"

    else
        value


{-| The live chart; lazy, so events logged elsewhere on the page do not touch it.
-}
stage : Data.Dataset -> String -> String -> String -> Bool -> String -> Html Msg
stage dataset t optionsJson theme mono rows =
    let
        attrs =
            elementAttributes t optionsJson { theme = theme, mono = mono, rows = rows } dataset
    in
    Keyed.node "div"
        [ A.class "stage" ]
        [ ( t
          , Plot.glyphPlot
                (A.id (plotId t)
                    :: List.map (\( k, v ) -> A.attribute k v) attrs
                    ++ Plot.onEvents (PlotEvent (plotId t))
                )
          )
        ]


controls : Meta -> List Field -> Types.ChartState -> Html Msg
controls meta fields st =
    Html.div [ A.class "controls" ]
        [ Html.fieldset []
            (Html.legend [] [ Html.text "Options" ]
                :: List.map (control st.inputs) fields
                ++ List.map (\p -> Html.p [ A.class "problem" ] [ Html.text p ]) (Options.problems fields st.inputs)
            )
        , Html.fieldset []
            [ Html.legend [] [ Html.text "Attributes" ]
            , Html.div [ A.class "control" ]
                [ Html.label [ A.for "attr-theme" ] [ Html.code [] [ Html.text "theme" ] ]
                , Html.select [ A.id "attr-theme", Events.onInput (SetAttr "theme") ]
                    (List.map (\v -> Html.option [ A.value v, A.selected (v == st.theme) ] [ Html.text v ]) [ "auto", "dark", "light", "paper" ])
                ]
            , Html.div [ A.class "control check" ]
                [ Html.input
                    [ A.type_ "checkbox"
                    , A.id "attr-mono"
                    , A.checked st.mono
                    , Events.onCheck
                        (\b ->
                            SetAttr "mono"
                                (if b then
                                    "true"

                                 else
                                    "false"
                                )
                        )
                    ]
                    []
                , Html.label [ A.for "attr-mono" ] [ Html.code [] [ Html.text "mono" ] ]
                ]
            , Html.div [ A.class "control" ]
                [ Html.label [ A.for "attr-rows" ] [ Html.code [] [ Html.text "rows" ] ]
                , Html.input
                    [ A.type_ "number"
                    , A.id "attr-rows"
                    , A.min "1"
                    , A.max "60"
                    , A.value st.rows
                    , Events.onInput (SetAttr "rows")
                    ]
                    []
                ]
            ]
        ]


control : Options.Inputs -> Field -> Html Msg
control inputs f =
    let
        id =
            "opt-" ++ f.path

        current =
            Options.inputOf inputs f

        label =
            Html.label [ A.for id ] [ Html.code [] [ Html.text f.path ] ]
    in
    case f.control of
        Choice values ->
            Html.div [ A.class "control" ]
                [ label
                , Html.select [ A.id id, Events.onInput (SetOption f.path) ]
                    (List.map (\v -> Html.option [ A.value v, A.selected (v == current) ] [ Html.text v ]) values)
                ]

        Toggle ->
            Html.div [ A.class "control check" ]
                [ Html.input
                    [ A.type_ "checkbox"
                    , A.id id
                    , A.checked (current == "true")
                    , Events.onCheck
                        (\b ->
                            SetOption f.path
                                (if b then
                                    "true"

                                 else
                                    "false"
                                )
                        )
                    ]
                    []
                , label
                ]

        Number ->
            Html.div [ A.class "control" ]
                [ label
                , Html.input [ A.type_ "number", A.id id, A.attribute "step" "any", A.value current, Events.onInput (SetOption f.path) ] []
                ]

        Text ->
            Html.div [ A.class "control" ]
                [ label
                , Html.input [ A.type_ "text", A.id id, A.value current, Events.onInput (SetOption f.path) ] []
                ]

        JsonText ->
            Html.div [ A.class "control wide" ]
                [ label
                , Html.textarea [ A.id id, A.rows 2, A.spellcheck False, A.value current, Events.onInput (SetOption f.path) ] []
                ]


exampleSection : Model -> String -> Html Msg
exampleSection model t =
    Html.section [ A.attribute "aria-labelledby" "example-h" ]
        (Html.h2 [ A.id "example-h" ] [ Html.text "An example" ]
            :: (case Manifest.examplesOfType t model.site.examples of
                    first :: rest ->
                        [ Html.h3 [] [ Html.a [ Ui.href model (Route.Example first.slug) ] [ Html.text first.title ] ]
                        , Html.p [] [ Html.text first.what ]
                        , Plot.example first.slug []
                        , Html.p []
                            [ Html.a [ Ui.href model (Route.Gallery { noFilter | type_ = Just t }) ]
                                [ Html.text ("Every " ++ t ++ " example (" ++ String.fromInt (1 + List.length rest) ++ ")") ]
                            ]
                        ]

                    [] ->
                        [ Html.p [ A.class "muted" ] [ Html.text ("The gallery has no " ++ t ++ " example yet.") ] ]
               )
        )


noFilter : Route.Filters
noFilter =
    Route.noFilters


htmlSnippet : String -> String -> Types.ChartState -> Data.Dataset -> String
htmlSnippet t optionsJson st dataset =
    let
        attr ( k, v ) =
            if k == "mono" then
                " mono"

            else if String.contains "\"" v then
                " " ++ k ++ "='" ++ v ++ "'"

            else
                " " ++ k ++ "=\"" ++ v ++ "\""

        attrs =
            elementAttributes t optionsJson st dataset
                |> List.filter (\( k, _ ) -> k /= "data" && k /= "label")
                |> List.map attr
                |> String.concat
    in
    String.join "\n"
        [ "<glyph-plot" ++ attrs ++ " label=\"My " ++ t ++ " chart\"></glyph-plot>"
        , ""
        , "<script type=\"module\">"
        , "  // \"glyphplot/element\" resolves through the installed web/ folder; a bundler serves it."
        , "  import \"glyphplot/element\";"
        , ""
        , "  const el = document.querySelector(\"glyph-plot\");"
        , "  // The columns shorthand: an array a column. Your rows, not these two."
        , "  el.data = " ++ st.sample2 ++ ";"
        , "  el.addEventListener(\"plot-hover\", (e) => console.log(e.detail));"
        , "</script>"
        ]


jsSnippet : String -> String -> Types.ChartState -> String
jsSnippet t optionsJson st =
    String.join "\n"
        [ "import { chart } from \"glyphplot\";"
        , ""
        , "const c = chart(\"" ++ t ++ "\", {"
        , "  data: " ++ st.sample3 ++ ","
        , "  options: " ++ optionsJson ++ ","
        , "  cols: 60,"
        , "  rows: 16,"
        , "});"
        , "await c.ready;   // the module loads once a page"
        , "c.frame(0);"
        , "console.log(c.text());   // the frame as one-ink text"
        , "c.free();"
        ]
