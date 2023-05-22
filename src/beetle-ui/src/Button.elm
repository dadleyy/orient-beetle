module Button exposing (..)

import Html
import Html.Attributes as A
import Html.Events as E
import Icon as I


type Button a
    = Icon I.Icon a
    | DisabledIcon I.Icon


view : Button a -> Html.Html a
view button =
    case button of
        Icon i onClick ->
            Html.button [ A.disabled False, E.onClick onClick ]
                [ I.view i ]

        DisabledIcon i ->
            Html.button [ A.disabled True ]
                [ I.view i ]
