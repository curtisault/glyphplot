module Plot exposing
    ( LogEntry
    , eventNames
    , example
    , glyphPlot
    , logView
    , onEvents
    , record
    )

{-| The two custom elements as Elm renders them.

`<glyph-plot>` takes everything Elm sets as attributes: `type`, `data`,
`options`, `view`, `theme`, `rows`, `label`, `mono` (REQ-EL-020). Elm compares
attribute strings, so the element sees a change only when one changes. Its
`plot-*` events are CustomEvents that bubble (REQ-EVENT-010), so Elm listens
to them on the element and reads `detail`. The element's methods and typed
arrays go through the `plotCall` port (Ports.elm).

`<gp-example slug="…">` is site/examples/element.ts's: it draws the
example's build-time still, then its live chart.

-}

import Html exposing (Attribute, Html)
import Html.Attributes as A
import Html.Events as Events
import Html.Keyed as Keyed
import Json.Decode as D exposing (Value)
import Json.Encode as E


{-| The events the element fires (REQ-EVENT-010).
-}
eventNames : List String
eventNames =
    [ "plot-ready", "plot-error", "plot-hover", "plot-select", "plot-view", "plot-cursor", "plot-brush", "plot-legend", "plot-data" ]


{-| A `<glyph-plot>`; the element keeps its own children (light DOM, REQ-EL-130), which Elm never touches.
-}
glyphPlot : List (Attribute msg) -> Html msg
glyphPlot attrs =
    Html.node "glyph-plot" attrs []


{-| A handler on every `plot-*` event, given the event's name and `detail`.
-}
onEvents : (String -> Value -> msg) -> List (Attribute msg)
onEvents toMsg =
    List.map
        (\name -> Events.on name (D.map (toMsg name) (D.oneOf [ D.field "detail" D.value, D.succeed E.null ])))
        eventNames


{-| An example's chart, keyed by slug so a different example gets a new element.
-}
example : String -> List (Attribute msg) -> Html msg
example slug attrs =
    Keyed.node "div"
        [ A.class "example-host" ]
        [ ( slug, Html.node "gp-example" (A.attribute "slug" slug :: attrs) [] ) ]


{-| One logged event.
-}
type alias LogEntry =
    { n : Int
    , plot : String
    , name : String
    , detail : String
    }


{-| Adds an event to a log kept newest first, at most `keep` long.
-}
record : Int -> LogEntry -> List LogEntry -> List LogEntry
record keep entry log =
    List.take keep (entry :: log)


{-| The event log: the latest events, newest first, each with its detail as JSON.
-}
logView : { title : String, clear : msg } -> List LogEntry -> Html msg
logView { title, clear } log =
    Html.section [ A.class "log", A.attribute "aria-label" title ]
        [ Html.div [ A.class "log-head" ]
            [ Html.h3 [] [ Html.text title ]
            , Html.button [ A.type_ "button", Events.onClick clear ] [ Html.text "Clear" ]
            ]
        , if List.isEmpty log then
            Html.p [ A.class "muted" ] [ Html.text "No events yet. Hover, drag, scroll with Ctrl or ⌘, or focus the chart and press the arrow keys." ]

          else
            Keyed.ol [ A.class "log-list" ]
                (List.map
                    (\e ->
                        ( String.fromInt e.n
                        , Html.li []
                            [ Html.code [ A.class "event-name" ] [ Html.text e.name ]
                            , Html.text " "
                            , Html.span [ A.class "event-detail" ] [ Html.text e.detail ]
                            ]
                        )
                    )
                    log
                )
        ]
