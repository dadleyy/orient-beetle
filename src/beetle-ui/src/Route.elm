module Route exposing (Message, Route(..), fromUrl, render)

import Environment
import Html
import Html.Attributes
import Url

import Route.Home


type Route
    = Login
    | Home


type Message
    = Blank
    | Phantom


render : Environment.Environment -> Route -> Html.Html Message
render env route =
    case route of
        Login ->
            Html.div [ Html.Attributes.class "px-4 py-3" ]
                [ Html.a [ Html.Attributes.href env.configuration.loginUrl ] [ Html.text "login" ]
                ]

        Home ->
            Html.div [ Html.Attributes.class "px-4 py-3" ] [ Html.text "home, yay" ]


fromUrl : Environment.Environment -> Url.Url -> Maybe Route
fromUrl env url =
    case ( url.path, Environment.getId env ) of
        ( "/login", _ ) ->
            Just Login

        ( "/home", Just _ ) ->
            Just Home

        _ ->
            Nothing
