module Types exposing
    ( ChartState
    , Model
    , Msg(..)
    , Source(..)
    , chartState
    , effectiveTheme
    )

{-| The site's model and messages, shared by every page module.
-}

import Browser
import Browser.Navigation as Nav
import Data
import Dict exposing (Dict)
import Json.Encode exposing (Value)
import Manifest exposing (Site, ThemeName)
import Options
import Plot exposing (LogEntry)
import Route exposing (Route)
import Url exposing (Url)


type alias Model =
    { key : Nav.Key
    , site : Site
    , route : Route
    , storedTheme : Maybe ThemeName
    , systemTheme : ThemeName
    , chart : ChartState
    , log : List LogEntry
    , logN : Int
    , views : Dict String String
    , replies : Dict String String
    , sources : Dict String Source
    , copied : Maybe String
    , problem : Maybe String
    }


{-| A chart type page's controls: the options typed or chosen, and the attributes set (REQ-SITE-020).
-}
type alias ChartState =
    { type_ : String
    , inputs : Options.Inputs
    , theme : String
    , mono : Bool
    , rows : String
    , dataset : Data.Dataset
    , sample2 : String
    , sample3 : String
    }


chartState : String -> ChartState
chartState t =
    let
        dataset =
            Data.forType t
    in
    { type_ = t
    , inputs = Dict.empty
    , theme = "auto"
    , mono = False
    , rows =
        if t == "sparkline" then
            "4"

        else
            "18"
    , dataset = dataset
    , sample2 = Data.sample 2 dataset
    , sample3 = Data.sample 3 dataset
    }


{-| An example module's source text, loaded on its page.
-}
type Source
    = Loading
    | Loaded String
    | Missing


type Msg
    = UrlRequested Browser.UrlRequest
    | UrlChanged Url
    | ChooseTheme ThemeName
    | FollowSystem
    | SystemTheme String
    | SetOption String String
    | SetAttr String String
    | ResetOptions
    | PlotEvent String String Value
    | ClearLog
    | CallPlot String String Value
    | PlotReply Value
    | Copy String String
    | Copied Value
    | GotSource Value
    | SetFilter String String
    | ClearCopied String
    | NoOp


{-| The theme in force: the reader's choice, else `prefers-color-scheme`.
-}
effectiveTheme : Model -> ThemeName
effectiveTheme model =
    Maybe.withDefault model.systemTheme model.storedTheme
