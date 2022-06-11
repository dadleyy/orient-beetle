module Route exposing (Message(..), Route(..), RouteInitialization(..), fromUrl, update, view)

import Environment
import Html
import Html.Attributes
import Route.Device
import Route.Home
import Url


type RouteInitialization
    = Matched ( Maybe Route, Cmd Message )
    | Redirect String


type Route
    = Login
    | Home Route.Home.Model
    | Device Route.Device.Model


type Message
    = HomeMessage Route.Home.Message
    | DeviceMessage Route.Device.Message
    | Phantom


view : Environment.Environment -> Route -> Html.Html Message
view env route =
    case route of
        Login ->
            Html.div [ Html.Attributes.class "px-4 py-3" ]
                [ Html.a [ Html.Attributes.href env.configuration.loginUrl ] [ Html.text "login" ]
                ]

        Home inner ->
            Route.Home.view inner env |> Html.map HomeMessage

        Device inner ->
            Route.Device.view inner env |> Html.map DeviceMessage


update : Environment.Environment -> Message -> Route -> ( Route, Cmd Message )
update env message route =
    case ( message, route ) of
        ( DeviceMessage deviceMessage, Device deviceModel ) ->
            let
                ( newDeviceModel, deviceCommand ) =
                    Route.Device.update env deviceMessage deviceModel
            in
            ( Device newDeviceModel, deviceCommand |> Cmd.map DeviceMessage )

        ( HomeMessage homeMessage, Home homeModel ) ->
            let
                ( newHome, homeCmd ) =
                    Route.Home.update env homeMessage homeModel
            in
            ( Home newHome, homeCmd |> Cmd.map HomeMessage )

        ( _, other ) ->
            ( other, Cmd.none )


fromUrl : Environment.Environment -> Url.Url -> RouteInitialization
fromUrl env url =
  let 
      normalizedUrl = Environment.normalizeUrlPath env url
  in
    case Environment.isLoaded env of
        False ->
            Matched ( Nothing, Cmd.none )

        True ->
            case String.startsWith "devices" normalizedUrl of
                True ->
                    case Environment.getId env of
                        Just _ ->
                            let
                                maybeDeviceId =
                                    normalizedUrl
                                        |> String.split "/"
                                        |> List.take 2
                                        |> List.tail
                                        -- |> Maybe.andThen List.tail
                                        |> Maybe.andThen List.head
                            in
                            case maybeDeviceId of
                                Just id ->
                                    let
                                        ( model, cmd ) =
                                            Route.Device.default env id
                                    in
                                    Matched ( Just (Device model), cmd |> Cmd.map DeviceMessage )

                                Nothing ->
                                    Redirect (Environment.buildRoutePath env "login")

                        Nothing ->
                            Redirect (Environment.buildRoutePath env "login")

                False ->
                    case ( normalizedUrl, Environment.getId env ) of
                        ( "login", Just _ ) ->
                            Redirect (Environment.buildRoutePath env "home")

                        ( "login", Nothing ) ->
                            Matched ( Just Login, Cmd.none )

                        ( "home", Just _ ) ->
                            let
                                ( route, cmd ) =
                                    Route.Home.default env
                            in
                            Matched ( Just (Home route), cmd |> Cmd.map HomeMessage )

                        _ ->
                            Matched ( Nothing, Cmd.none )
