module Route exposing (Message(..), Route(..), RouteInitialization(..), fromUrl, subscriptions, update, view)

import Environment
import Html
import Html.Attributes
import Html.Parser
import Html.Parser.Util as HTP
import Route.Device
import Route.DeviceRegistration
import Route.Home
import Url


type RouteInitialization
    = Matched ( Maybe Route, Cmd Message )
    | Redirect String


type Route
    = Login
    | Home Route.Home.Model
    | Device Route.Device.Model
    | DeviceRegistration Route.DeviceRegistration.Model


type Message
    = HomeMessage Route.Home.Message
    | DeviceMessage Route.Device.Message
    | DeviceRegistrationMessage Route.DeviceRegistration.Message


subscriptions : Route -> Sub Message
subscriptions route =
    case route of
        Device deviceModel ->
            Sub.map DeviceMessage (Route.Device.subscriptions deviceModel)

        _ ->
            Sub.none


view : Environment.Environment -> Route -> Html.Html Message
view env route =
    case route of
        Login ->
            Html.div [ Html.Attributes.class "px-4 py-3" ]
                [ renderLogin env ]

        Home inner ->
            Route.Home.view inner env |> Html.map HomeMessage

        Device inner ->
            Route.Device.view inner env |> Html.map DeviceMessage

        DeviceRegistration inner ->
            Route.DeviceRegistration.view env inner |> Html.map DeviceRegistrationMessage


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

        ( DeviceRegistrationMessage registrationMessage, DeviceRegistration registrationModel ) ->
            let
                ( newReg, regCmd ) =
                    Route.DeviceRegistration.update env registrationMessage registrationModel
            in
            ( DeviceRegistration newReg, regCmd |> Cmd.map DeviceRegistrationMessage )

        ( _, other ) ->
            ( other, Cmd.none )


justIfNotEmpty : String -> Maybe String
justIfNotEmpty input =
    case String.isEmpty input of
        True ->
            Nothing

        False ->
            Just input


deviceRouting : Environment.Environment -> String -> RouteInitialization
deviceRouting env normalizedUrl =
    let
        maybeDeviceId =
            normalizedUrl
                |> String.split "/"
                |> List.take 2
                |> List.tail
                |> Maybe.andThen List.head
    in
    case Maybe.map2 Tuple.pair (Environment.getId env) (maybeDeviceId |> Maybe.andThen justIfNotEmpty) of
        Just matches ->
            let
                ( model, cmd ) =
                    Route.Device.default env (Tuple.second matches)
            in
            Matched ( Just (Device model), cmd |> Cmd.map DeviceMessage )

        Nothing ->
            Redirect (Environment.buildRoutePath env "login")


routeLoadedEnv : Environment.Environment -> String -> Maybe String -> RouteInitialization
routeLoadedEnv env normalizedUrl maybeId =
    --  TODO: is there a better way to navigate/route subroute?
    case String.startsWith "devices" normalizedUrl of
        True ->
            deviceRouting env normalizedUrl

        False ->
            case ( normalizedUrl, maybeId ) of
                ( "register-device", Just _ ) ->
                    Matched ( Just (DeviceRegistration Route.DeviceRegistration.default), Cmd.none )

                ( "login", Just _ ) ->
                    Redirect (Environment.buildRoutePath env "home")

                ( "login", Nothing ) ->
                    Matched ( Just Login, Cmd.none )

                ( "home", Nothing ) ->
                    Redirect (Environment.buildRoutePath env "login")

                ( "home", Just _ ) ->
                    let
                        ( route, cmd ) =
                            Route.Home.default env
                    in
                    Matched ( Just (Home route), cmd |> Cmd.map HomeMessage )

                _ ->
                    Redirect (Environment.buildRoutePath env "home")


fromUrl : Environment.Environment -> Url.Url -> RouteInitialization
fromUrl env url =
    let
        normalizedUrl =
            Environment.normalizeUrlPath env url
    in
    Environment.getLoadedId env
        |> Maybe.map (routeLoadedEnv env normalizedUrl)
        |> Maybe.withDefault (Matched ( Nothing, Cmd.none ))


renderLogin : Environment.Environment -> Html.Html Message
renderLogin env =
    let
        loginContentText =
            Maybe.withDefault "" (Environment.getLocalizedContent env "login_page")

        loginContentDom =
            Result.withDefault [] (Result.map HTP.toVirtualDom (Html.Parser.run loginContentText))
    in
    Html.div [ Html.Attributes.class "flex items-start" ]
        [ Html.div [ Html.Attributes.class "flex-1 pr-3" ] loginContentDom
        , Html.div [ Html.Attributes.class "flex-1 pl-3" ]
            [ Html.div []
                [ Html.a
                    [ Html.Attributes.href env.configuration.loginUrl
                    , Html.Attributes.rel "noopener"
                    , Html.Attributes.target "_self"
                    ]
                    [ Html.text "Login" ]
                ]
            ]
        ]
