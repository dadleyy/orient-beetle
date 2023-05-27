module Alert exposing (Alert(..), view)

import Html
import Html.Attributes as A


type Alert
    = Warning String
    | Happy String


view : Alert -> Html.Html a
view alert =
    case alert of
        Warning content ->
            Html.div [ A.class "alert-warning" ] [ Html.text content ]

        Happy content ->
            Html.div [] []
