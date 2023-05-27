module Icon exposing (..)

import Html
import Html.Attributes as A


type Icon
    = Github
    | Docs
    | Home
    | Trash
    | Cancel
    | Lightbulb
    | Pencil
    | Link
    | File
    | Moon
    | Sun
    | Add
    | Send
    | EllipsisH


view : Icon -> Html.Html a
view icon =
    case icon of
        Cancel ->
            Html.i [ A.class "icon-close" ] []

        EllipsisH ->
            Html.i [ A.class "icon-ellipsis-h" ] []

        Pencil ->
            Html.i [ A.class "icon-pencil" ] []

        Link ->
            Html.i [ A.class "icon-link" ] []

        File ->
            Html.i [ A.class "icon-file" ] []

        Moon ->
            Html.i [ A.class "icon-moon-o" ] []

        Sun ->
            Html.i [ A.class "icon-sun-o" ] []

        Send ->
            Html.i [ A.class "icon-send" ] []

        Lightbulb ->
            Html.i [ A.class "icon-lightbulb-o" ] []

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
