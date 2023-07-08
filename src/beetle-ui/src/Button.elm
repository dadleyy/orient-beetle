module Button exposing (..)

import Html
import Html.Attributes as A
import Html.Events as E
import Icon as I


type Button a
    = PrimaryIcon I.Icon a
    | SecondaryIcon I.Icon a
    | LinkIcon I.Icon String
    | DisabledIcon I.Icon


disable : Button a -> Button a
disable button =
    case button of
        LinkIcon i url ->
            DisabledIcon i

        SecondaryIcon i _ ->
            DisabledIcon i

        PrimaryIcon i _ ->
            DisabledIcon i

        DisabledIcon i ->
            DisabledIcon i


view : Button a -> Html.Html a
view button =
    case button of
        LinkIcon i url ->
            Html.a [ A.href url, A.class "icon-container link-button" ]
                [ I.view i ]

        SecondaryIcon i onClick ->
            Html.button [ A.class "icon-container secondary-button", A.disabled False, E.onClick onClick ]
                [ I.view i ]

        PrimaryIcon i onClick ->
            Html.button [ A.class "icon-container", A.disabled False, E.onClick onClick ]
                [ I.view i ]

        DisabledIcon i ->
            Html.button [ A.disabled True, A.class "icon-container" ]
                [ I.view i ]
