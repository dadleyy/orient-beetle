module Icon exposing (..)

import Html
import Html.Attributes as A


type Icon
    = Github
    | Docs
    | Home
    | Trash
    | Cancel
    | CalendarOn
    | CalendarOff
    | Lightbulb
    | UserSecret
    | User
    | Unlock
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
        CalendarOn ->
            Html.i [ A.class "icon-calendar-times-o" ] []

        CalendarOff ->
            Html.i [ A.class "icon-calendar-check-o" ] []

        UserSecret ->
            Html.i [ A.class "icon-user-secret" ] []

        Unlock ->
            Html.i [ A.class "icon-unlock" ] []

        User ->
            Html.i [ A.class "icon-user" ] []

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
