port module Ports exposing
    ( copied
    , copy
    , gotSource
    , plotCall
    , plotReply
    , requestSource
    , setTheme
    , systemTheme
    )

{-| What Elm cannot do itself, done by site/src/main.ts.

Elm sets a `<glyph-plot>`'s attributes and listens to its `plot-*` events
directly; these ports cover the rest: the theme on `<html>` and in
localStorage, the element's methods and typed-array data, the clipboard, and
an example's module source.

-}

import Json.Encode exposing (Value)


{-| "light", "dark" or "system": sets or removes `data-theme` on `<html>` and keeps the choice in localStorage (REQ-SITE-060).
-}
port setTheme : String -> Cmd msg


{-| The reader's `prefers-color-scheme`, "light" or "dark", on load and whenever it changes.
-}
port systemTheme : (String -> msg) -> Sub msg


{-| `{ id, method, args }`: a method on the `<glyph-plot>` of that id (`resetView`, `text`, `fill`).
-}
port plotCall : Value -> Cmd msg


{-| `{ id, method, value }`: what a `plotCall` returned.
-}
port plotReply : (Value -> msg) -> Sub msg


{-| `{ id, text }`: writes the text to the clipboard.
-}
port copy : Value -> Cmd msg


{-| `{ id, ok }`: whether the copy took.
-}
port copied : (Value -> msg) -> Sub msg


{-| An example's slug: main.ts loads site/examples/<slug>.ts as text (Vite's `?raw`).
-}
port requestSource : String -> Cmd msg


{-| `{ slug, text }`, `text` null when the example has no module.
-}
port gotSource : (Value -> msg) -> Sub msg
