module Alert exposing (Alert(..), view)

import Button
import Html
import Html.Attributes as A
import Icon


type Alert
    = Warning String
    | Happy String


view : Alert -> a -> Html.Html a
view alert message =
    case alert of
        Warning content ->
            Html.div [ A.class "alert-warning flex items-center w-full" ]
                [ Html.div [ A.class "flex-1" ] [ Html.text content ]
                , Html.div [ A.class "ml-auto" ] [ Button.view (Button.SecondaryIcon Icon.Cancel message) ]
                ]

        Happy content ->
            Html.div [] []
