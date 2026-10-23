module Main exposing (main)

{-| glyphplot's site (REQ-SITE-010, ADR-143): Elm 0.19.1 under Vite, its
flags read by site/src/main.ts from web/pkg/modules.js (through the shell's
`manifest()`), web/pkg/sizes.json and site/examples/manifest.json. No wasm is
loaded for the page to show: each `<glyph-plot>` loads its family's module
when it nears the screen.
-}

import Browser
import Browser.Dom as Dom
import Browser.Navigation as Nav
import ChartPage
import Data
import Dict
import Docs
import Gallery
import Html exposing (Html)
import Html.Attributes as A
import Html.Events as Events
import Json.Decode as D
import Json.Encode as E
import Manifest exposing (Site, ThemeName(..))
import Plot
import Ports
import Process
import Route exposing (Route(..))
import Task
import Types exposing (Model, Msg(..), Source(..))
import Ui
import Url exposing (Url)


main : Program D.Value Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        , onUrlRequest = UrlRequested
        , onUrlChange = UrlChanged
        }


emptySite : Site
emptySite =
    { charts = [], sizes = Dict.empty, examples = [], base = "/", storedTheme = Nothing, systemTheme = Light }


init : D.Value -> Url -> Nav.Key -> ( Model, Cmd Msg )
init flags url key =
    let
        ( site, problem ) =
            case D.decodeValue Manifest.decoder flags of
                Ok s ->
                    ( s, Nothing )

                Err e ->
                    ( emptySite, Just (D.errorToString e) )

        model =
            { key = key
            , site = site
            , route = NotFound
            , storedTheme = site.storedTheme
            , systemTheme = site.systemTheme
            , chart = Types.chartState ""
            , log = []
            , logN = 0
            , views = Dict.empty
            , replies = Dict.empty
            , sources = Dict.empty
            , copied = Nothing
            , problem = problem
            }

        route =
            Route.fromUrl site.base url
    in
    -- The first page is entered as the page already shown: no scroll and no
    -- focus move, which would count as the reader's act and wake the
    -- gallery's cards before the reader does anything (REQ-SITE-120).
    enter route { model | route = route }


{-| Moves to a route: a type page's controls start at their defaults, an example's source is asked for.
-}
enter : Route -> Model -> ( Model, Cmd Msg )
enter route model =
    let
        samePage =
            case ( route, model.route ) of
                ( Gallery _, Gallery _ ) ->
                    True

                _ ->
                    route == model.route

        fresh =
            if samePage then
                model

            else
                { model | log = [], views = Dict.empty, replies = Dict.empty }

        withRoute =
            { fresh | route = route }
    in
    case route of
        ChartType t ->
            ( if model.chart.type_ == t then
                withRoute

              else
                { withRoute | chart = Types.chartState t }
            , focusMain samePage
            )

        Example slug ->
            if Dict.member slug model.sources then
                ( withRoute, focusMain samePage )

            else
                ( { withRoute | sources = Dict.insert slug Loading model.sources }
                , Cmd.batch [ Ports.requestSource slug, focusMain samePage ]
                )

        _ ->
            ( withRoute, focusMain samePage )


{-| On a new page: to the top, and focus to the main region so a screen reader starts there.
-}
focusMain : Bool -> Cmd Msg
focusMain samePage =
    if samePage then
        Cmd.none

    else
        Cmd.batch
            [ Task.attempt (\_ -> NoOp) (Dom.setViewport 0 0)
            , Task.attempt (\_ -> NoOp) (Dom.focus "main")
            ]


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        UrlRequested (Browser.Internal url) ->
            if String.endsWith ".html" url.path || Route.fromUrl model.site.base url == NotFound then
                ( model, Nav.load (Url.toString url) )

            else
                ( model, Nav.pushUrl model.key (Url.toString url) )

        UrlRequested (Browser.External href) ->
            ( model, Nav.load href )

        UrlChanged url ->
            enter (Route.fromUrl model.site.base url) model

        ChooseTheme t ->
            ( { model | storedTheme = Just t }, Ports.setTheme (Manifest.themeToString t) )

        FollowSystem ->
            ( { model | storedTheme = Nothing }, Ports.setTheme "system" )

        SystemTheme s ->
            ( { model | systemTheme = Manifest.themeFromString s |> Maybe.withDefault model.systemTheme }, Cmd.none )

        SetOption path text ->
            let
                c =
                    model.chart
            in
            ( { model | chart = { c | inputs = Dict.insert path text c.inputs } }, Cmd.none )

        SetAttr name value ->
            let
                c =
                    model.chart
            in
            ( { model
                | chart =
                    case name of
                        "theme" ->
                            { c | theme = value }

                        "mono" ->
                            { c | mono = value == "true" }

                        "rows" ->
                            if String.toInt value |> Maybe.map (\n -> n >= 1 && n <= 60) |> Maybe.withDefault False then
                                { c | rows = value }

                            else
                                c

                        _ ->
                            c
              }
            , Cmd.none
            )

        ResetOptions ->
            let
                c =
                    model.chart
            in
            ( { model | chart = { c | inputs = Dict.empty } }, Cmd.none )

        PlotEvent id name detail ->
            let
                text =
                    E.encode 0 detail

                entry =
                    { n = model.logN + 1
                    , plot = id
                    , name = name
                    , detail =
                        if String.length text > 240 then
                            String.left 240 text ++ " …"

                        else
                            text
                    }

                views =
                    if name == "plot-view" then
                        case D.decodeValue (D.field "view" D.value) detail of
                            Ok v ->
                                Dict.insert id (E.encode 2 v) model.views

                            Err _ ->
                                model.views

                    else
                        model.views
            in
            ( { model | log = Plot.record 40 entry model.log, logN = model.logN + 1, views = views }, Cmd.none )

        ClearLog ->
            ( { model | log = [] }, Cmd.none )

        CallPlot id method args ->
            ( model, Ports.plotCall (E.object [ ( "id", E.string id ), ( "method", E.string method ), ( "args", args ) ]) )

        PlotReply v ->
            case D.decodeValue (D.map3 (\a b c -> ( a, b, c )) (D.field "id" D.string) (D.field "method" D.string) (D.field "value" D.string)) v of
                Ok ( id, method, value ) ->
                    ( { model | replies = Dict.insert (id ++ "/" ++ method) value model.replies }, Cmd.none )

                Err _ ->
                    ( model, Cmd.none )

        Copy id text ->
            ( model, Ports.copy (E.object [ ( "id", E.string id ), ( "text", E.string text ) ]) )

        Copied v ->
            case D.decodeValue (D.map2 Tuple.pair (D.field "id" D.string) (D.field "ok" D.bool)) v of
                Ok ( id, True ) ->
                    ( { model | copied = Just id }, Task.perform (\_ -> ClearCopied id) (Process.sleep 2000) )

                _ ->
                    ( model, Cmd.none )

        ClearCopied id ->
            ( if model.copied == Just id then
                { model | copied = Nothing }

              else
                model
            , Cmd.none
            )

        GotSource v ->
            case D.decodeValue (D.map2 Tuple.pair (D.field "slug" D.string) (D.field "text" (D.nullable D.string))) v of
                Ok ( slug, Just text ) ->
                    ( { model | sources = Dict.insert slug (Loaded text) model.sources }, Cmd.none )

                Ok ( slug, Nothing ) ->
                    ( { model | sources = Dict.insert slug Missing model.sources }, Cmd.none )

                Err _ ->
                    ( model, Cmd.none )

        SetFilter key value ->
            case model.route of
                Gallery filters ->
                    let
                        next =
                            if key == "" then
                                Route.noFilters

                            else
                                Route.setFilter key value filters
                    in
                    ( model, Nav.replaceUrl model.key (Route.toPath model.site.base (Gallery next)) )

                _ ->
                    ( model, Cmd.none )

        NoOp ->
            ( model, Cmd.none )


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.batch
        [ Ports.systemTheme SystemTheme
        , Ports.plotReply PlotReply
        , Ports.copied Copied
        , Ports.gotSource GotSource
        ]



-- VIEW


view : Model -> Browser.Document Msg
view model =
    let
        ( title, content ) =
            page model
    in
    { title =
        if title == "" then
            "glyphplot"

        else
            title ++ " · glyphplot"
    , body =
        [ Html.a [ A.class "skip", A.href "#main" ] [ Html.text "Skip to content" ]
        , header model
        , Html.div [ A.class "layout" ]
            [ sidebar model
            , Html.main_ [ A.id "main", A.tabindex -1 ]
                (case model.problem of
                    Just p ->
                        Html.p [ A.class "notice" ] [ Html.text ("The site's flags did not decode: " ++ p) ] :: content

                    Nothing ->
                        content
                )
            ]
        , Html.footer [ A.class "site-footer" ]
            [ Html.p []
                (Ui.inline "glyphplot is MIT-licensed and not yet published to npm or crates.io. Data on this site is generated unless a page names its source. The checklist page is "
                    ++ [ Html.a [ A.href (Route.toPath model.site.base Home ++ "test.html") ] [ Html.text "test.html" ], Html.text "." ]
                )
            ]
        ]
    }


header : Model -> Html Msg
header model =
    let
        current =
            Types.effectiveTheme model

        other =
            case current of
                Light ->
                    Dark

                Dark ->
                    Light

        link route label =
            Html.a
                (Ui.href model route
                    :: (if isCurrent model.route route then
                            [ A.attribute "aria-current" "page" ]

                        else
                            []
                       )
                )
                [ Html.text label ]
    in
    Html.header [ A.class "site-header" ]
        [ Html.a [ A.class "brand", Ui.href model Home ] [ Html.span [ A.class "mark", A.attribute "aria-hidden" "true" ] [ Html.text "⣠⣴⣾" ], Html.text "glyphplot" ]
        , Html.nav [ A.attribute "aria-label" "Site" ]
            [ Html.ul []
                (List.map (\m -> Html.li [] [ link (ChartType m.slug) m.slug ]) model.site.charts
                    ++ [ Html.li [] [ link (Doc "getting-started") "Docs" ]
                       , Html.li [] [ link (Gallery Route.noFilters) "Examples" ]
                       ]
                )
            ]
        , Html.div [ A.class "theme" ]
            (Html.button
                [ A.type_ "button"
                , A.class "theme-button"
                , Events.onClick (ChooseTheme other)
                , A.attribute "aria-label" ("Theme: " ++ Manifest.themeToString current ++ ". Switch to " ++ Manifest.themeToString other)
                ]
                [ Html.span [ A.attribute "aria-hidden" "true" ]
                    [ Html.text
                        (case other of
                            Dark ->
                                "☾ "

                            Light ->
                                "☀ "
                        )
                    ]
                , Html.text (capitalise (Manifest.themeToString other))
                ]
                :: (case model.storedTheme of
                        Just _ ->
                            [ Html.button [ A.type_ "button", A.class "link-button", Events.onClick FollowSystem ] [ Html.text "Follow system" ] ]

                        Nothing ->
                            []
                   )
            )
        ]


isCurrent : Route -> Route -> Bool
isCurrent at route =
    case ( at, route ) of
        ( Gallery _, Gallery _ ) ->
            True

        ( Example _, Gallery _ ) ->
            True

        ( Doc _, Doc _ ) ->
            True

        _ ->
            at == route


capitalise : String -> String
capitalise s =
    String.toUpper (String.left 1 s) ++ String.dropLeft 1 s


sidebar : Model -> Html Msg
sidebar model =
    case model.route of
        Doc current ->
            Html.nav [ A.class "sidebar", A.attribute "aria-label" "Docs" ]
                [ Html.ul []
                    (List.map
                        (\( slug, title ) ->
                            Html.li []
                                [ Html.a
                                    (Ui.href model (Doc slug)
                                        :: (if slug == current then
                                                [ A.attribute "aria-current" "page" ]

                                            else
                                                []
                                           )
                                    )
                                    [ Html.text title ]
                                ]
                        )
                        Route.docPages
                    )
                ]

        _ ->
            Html.text ""


page : Model -> ( String, List (Html Msg) )
page model =
    case model.route of
        Home ->
            ( "", home model )

        ChartType t ->
            case Manifest.metaOf t model.site.charts of
                Just meta ->
                    ( t, ChartPage.view model meta )

                Nothing ->
                    notFound model

        Doc slug ->
            case Docs.view model slug of
                Just content ->
                    ( List.filter (\( s, _ ) -> s == slug) Route.docPages |> List.head |> Maybe.map Tuple.second |> Maybe.withDefault slug
                    , content
                    )

                Nothing ->
                    notFound model

        Gallery filters ->
            ( "Examples", Gallery.view model filters )

        Example slug ->
            case Manifest.exampleOf slug model.site.examples of
                Just e ->
                    ( e.title, Gallery.examplePage model e )

                Nothing ->
                    notFound model

        NotFound ->
            notFound model


notFound : Model -> ( String, List (Html Msg) )
notFound model =
    ( "Not found"
    , [ Html.h1 [] [ Html.text "Not found" ]
      , Html.p []
            [ Html.text "No page here. Try "
            , Html.a [ Ui.href model Home ] [ Html.text "the front page" ]
            , Html.text ", "
            , Html.a [ Ui.href model (Doc "getting-started") ] [ Html.text "getting started" ]
            , Html.text " or "
            , Html.a [ Ui.href model (Gallery Route.noFilters) ] [ Html.text "the examples" ]
            , Html.text "."
            ]
      ]
    )


homeSet : Data.Dataset
homeSet =
    Data.series 11 240


home : Model -> List (Html Msg)
home model =
    [ Html.h1 [ A.class "hero" ] [ Html.text "glyphplot" ]
    , Html.p [ A.class "lede" ] (Ui.inline "Interactive charts drawn in Unicode glyphs: braille, blocks and box drawing, one code point and one colour a cell. The charts are Rust compiled to WebAssembly, one module a chart family, played in the browser by one tag, `<glyph-plot>`.")
    , Html.div [ A.class "stage" ]
        [ Plot.glyphPlot
            [ A.id "home-plot"
            , A.attribute "type" "line"
            , A.attribute "rows" "16"
            , A.attribute "label" "Three simulated daily series"
            , A.attribute "data" homeSet.json
            ]
        ]
    , Html.p [ A.class "muted" ]
        [ Ui.simulated homeSet.seed
        , Html.text " Hover for a readout, drag to pan, Ctrl or ⌘ and the wheel to zoom, or Tab to the chart and use the arrow keys."
        ]
    , Html.p [ A.class "notice" ]
        [ Html.text "Not published yet: there is no npm or crates.io package. "
        , Html.a [ Ui.href model (Doc "getting-started") ] [ Html.text "Getting started" ]
        , Html.text " shows how to build it from the repository."
        ]
    , Html.h2 [] [ Html.text "Chart types" ]
    , Html.ul [ A.class "type-cards" ]
        (List.map
            (\m ->
                Html.li []
                    [ Html.h3 [] [ Html.a [ Ui.href model (ChartType m.slug) ] [ Html.code [] [ Html.text m.slug ] ] ]
                    , Html.p [] [ Html.text (capitalise m.note ++ ".") ]
                    , Html.p [ A.class "muted" ] [ Html.text ("Modes: " ++ String.join ", " m.modes ++ ". Module: " ++ m.module_ ++ ".") ]
                    ]
            )
            model.site.charts
        )
    , Html.h2 [] [ Html.text "Documentation" ]
    , Html.ul [ A.class "doc-list" ]
        (List.map (\( slug, title ) -> Html.li [] [ Html.a [ Ui.href model (Doc slug) ] [ Html.text title ] ]) Route.docPages)
    , Html.h2 [] [ Html.text "Examples" ]
    , Html.p []
        [ Html.a [ Ui.href model (Gallery Route.noFilters) ]
            [ Html.text
                (if List.isEmpty model.site.examples then
                    "The gallery"

                 else
                    "The gallery of " ++ String.fromInt (List.length model.site.examples) ++ " examples"
                )
            ]
        , Html.text ", each a live chart with the source it runs."
        ]
    ]
