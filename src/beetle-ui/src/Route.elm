module Route exposing (Message(..), Route(..), fromUrl, update, view)

import Environment
import Html
import Html.Attributes
import Route.Home
import Url


type Route
    = Login
    | Home Route.Home.Model


type Message
    = HomeMessage Route.Home.Message
    | Phantom


view : Environment.Environment -> Route -> Html.Html Message
view env route =
    case route of
        Login ->
            Html.div [ Html.Attributes.class "px-4 py-3" ]
                [ Html.a [ Html.Attributes.href env.configuration.loginUrl ] [ Html.text "login" ]
                ]

        Home inner ->
            Route.Home.view inner |> Html.map HomeMessage


update : Environment.Environment -> Message -> Route -> ( Route, Cmd Message )
update env message route =
    case ( message, route ) of
        ( HomeMessage homeMessage, Home homeModel ) ->
            let
                ( newHome, homeCmd ) =
                    Route.Home.update env homeMessage homeModel
            in
            ( Home newHome, homeCmd |> Cmd.map HomeMessage )

        ( _, other ) ->
            ( other, Cmd.none )


fromUrl : Environment.Environment -> Url.Url -> ( Maybe Route, Cmd Message )
fromUrl env url =
    case ( url.path, Environment.getId env ) of
        ( "/login", _ ) ->
            ( Just Login, Cmd.none )

        ( "/home", Just _ ) ->
            let
                ( route, cmd ) =
                    Route.Home.default env
            in
            ( Just (Home route), cmd |> Cmd.map HomeMessage )

        _ ->
            ( Nothing, Cmd.none )
