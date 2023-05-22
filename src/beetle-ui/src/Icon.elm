module Icon exposing (..)

import Html
import Html.Attributes as A


type Icon
    = Github
    | Docs
    | Home
    | Trash
    | Add


view : Icon -> Html.Html a
view icon =
    case icon of
        Add ->
            Html.i [ A.class "icon-plus" ] []

        Trash ->
            Html.i [ A.class "icon-trash-o" ] []

        Home ->
            Html.i [ A.class "icon-home" ] []

        Docs ->
            Html.i [ A.class "icon-font" ] []

        Github ->
            Html.i [ A.class "icon-github-square" ] []


link : Icon -> String -> Html.Html a
link icon destination =
    Html.a [ A.href destination ] [ view icon ]
