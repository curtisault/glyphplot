module Gallery exposing (examplePage, view)

{-| The examples page is the gallery (REQ-SITE-040): a card per manifest
entry, each a `<gp-example>` (a build-time still until it nears the screen,
then live), its title, its one line, its types, modes, domain and features,
filterable by each and by milestone, linking to the example's own page. The
example's page shows it full size with the module source it runs, copyable.
-}

import Dict
import Html exposing (Html)
import Html.Attributes as A
import Html.Events as Events
import Html.Keyed as Keyed
import Manifest exposing (Example)
import Plot
import Route exposing (Filters)
import Types exposing (Model, Msg(..), Source(..))
import Ui


view : Model -> Filters -> List (Html Msg)
view model filters =
    let
        all =
            model.site.examples

        shown =
            List.filter (matches filters) all
    in
    [ Html.h1 [] [ Html.text "Examples" ]
    , Html.p [ A.class "lede" ] [ Html.text "Every example is a live chart: a still drawn at build time until it comes near the screen, then the chart itself, loading its family's module once however many cards use it." ]
    , filterForm all filters
    , Html.p [ A.class "muted", A.attribute "role" "status" ]
        [ Html.text
            (if List.isEmpty all then
                "The gallery has no examples yet."

             else
                String.fromInt (List.length shown) ++ " of " ++ String.fromInt (List.length all) ++ " examples"
            )
        ]
    , Keyed.node "ul" [ A.class "cards" ] (List.map (\e -> ( e.slug, card model e )) shown)
    ]


matches : Filters -> Example -> Bool
matches f e =
    let
        ok want has =
            case want of
                Nothing ->
                    True

                Just w ->
                    has w
    in
    ok f.type_ (\w -> List.member w e.types)
        && ok f.mode (\w -> List.member w e.modes)
        && ok f.domain (\w -> w == e.domain)
        && ok f.feature (\w -> List.member w e.features)
        && ok f.milestone (\w -> w == e.milestone)


valuesOf : String -> List Example -> List String
valuesOf key examples =
    examples
        |> List.concatMap
            (\e ->
                case key of
                    "type" ->
                        e.types

                    "mode" ->
                        e.modes

                    "domain" ->
                        [ e.domain ]

                    "feature" ->
                        e.features

                    "milestone" ->
                        [ e.milestone ]

                    _ ->
                        []
            )
        |> List.filter ((/=) "")
        |> List.sort
        |> dedupe


dedupe : List String -> List String
dedupe =
    List.foldr
        (\x acc ->
            case acc of
                y :: _ ->
                    if x == y then
                        acc

                    else
                        x :: acc

                [] ->
                    [ x ]
        )
        []


filterForm : List Example -> Filters -> Html Msg
filterForm all filters =
    Html.div [ A.class "filters", A.attribute "role" "group", A.attribute "aria-label" "Filter the examples" ]
        (List.map
            (\key ->
                let
                    id =
                        "filter-" ++ key

                    current =
                        Route.getFilter key filters |> Maybe.withDefault ""
                in
                Html.div [ A.class "control" ]
                    [ Html.label [ A.for id ] [ Html.text (labelOf key) ]
                    , Html.select [ A.id id, Events.onInput (SetFilter key) ]
                        (Html.option [ A.value "", A.selected (current == "") ] [ Html.text "any" ]
                            :: List.map (\v -> Html.option [ A.value v, A.selected (v == current) ] [ Html.text v ]) (valuesOf key all)
                        )
                    ]
            )
            Route.filterKeys
            ++ [ Html.button [ A.type_ "button", Events.onClick (SetFilter "" "") ] [ Html.text "Clear filters" ] ]
        )


labelOf : String -> String
labelOf key =
    case key of
        "type" ->
            "Chart type"

        "mode" ->
            "Mode"

        "domain" ->
            "Domain"

        "feature" ->
            "Feature"

        _ ->
            "Milestone"


{-| Where the data comes from: "simulated" with the seed (REQ-SITE-110), or the live feed's source and licence (REQ-SITE-050).
-}
provenance : Example -> Html msg
provenance e =
    case ( e.feed, e.seed ) of
        ( Just feed, _ ) ->
            Html.p [ A.class "provenance" ]
                [ Html.text "Live feed: "
                , Html.a [ A.href feed.url ] [ Html.text feed.source ]
                , Html.text (", " ++ feed.licence)
                ]

        ( Nothing, Just seed ) ->
            Html.p [ A.class "provenance" ] [ Ui.simulated seed ]

        ( Nothing, Nothing ) ->
            if List.isEmpty e.datasets then
                Html.text ""

            else
                Html.p [ A.class "provenance" ] [ Html.text ("Data: " ++ String.join ", " e.datasets) ]


tags : Example -> Html msg
tags e =
    Html.dl [ A.class "tags" ]
        (List.concatMap
            (\( k, vs ) ->
                if List.isEmpty vs then
                    []

                else
                    [ Html.dt [] [ Html.text k ], Html.dd [] [ Html.text (String.join ", " vs) ] ]
            )
            [ ( "Type", e.types )
            , ( "Modes", e.modes )
            , ( "Domain", List.filter ((/=) "") [ e.domain ] )
            , ( "Features", e.features )
            , ( "Milestone", List.filter ((/=) "") [ e.milestone ] )
            ]
        )


card : Model -> Example -> Html Msg
card model e =
    Html.li [ A.class "card" ]
        [ Plot.example e.slug []
        , Html.h2 [] [ Html.a [ Ui.href model (Route.Example e.slug) ] [ Html.text e.title ] ]
        , Html.p [] [ Html.text e.what ]
        , provenance e
        , tags e
        ]


{-| An example's own page: the chart full size and the source it runs (REQ-SITE-040).
-}
examplePage : Model -> Example -> List (Html Msg)
examplePage model e =
    let
        file =
            "site/examples/" ++ e.slug ++ ".ts"
    in
    [ Html.p [ A.class "crumbs" ] [ Html.a [ Ui.href model (Route.Gallery Route.noFilters) ] [ Html.text "Examples" ], Html.text " / " ]
    , Html.h1 [] [ Html.text e.title ]
    , Html.p [ A.class "lede" ] [ Html.text e.what ]
    , provenance e
    , Plot.example e.slug [ A.class "full" ]
    , tags e
    , Html.section [ A.attribute "aria-labelledby" "source-h" ]
        [ Html.h2 [ A.id "source-h" ] [ Html.text "The source it runs" ]
        , case Dict.get e.slug model.sources of
            Just (Loaded text) ->
                Ui.codeBlock model ("source-" ++ e.slug) file text

            Just Missing ->
                Html.p [ A.class "muted" ] [ Html.text (file ++ " is not in this build.") ]

            _ ->
                Html.p [ A.class "muted" ] [ Html.text ("Loading " ++ file ++ " …") ]
        ]
    ]
