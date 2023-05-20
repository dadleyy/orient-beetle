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
import Url.Parser as UrlParser exposing ((</>))
import Url.Parser.Query as QueryParser


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
            Html.div [ Html.Attributes.class "px-4 py-3 h-full w-full relative" ]
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


routeLoadedEnv : Environment.Environment -> Url.Url -> Maybe String -> RouteInitialization
routeLoadedEnv env url maybeId =
    let
        normalizedUrl =
            Environment.normalizeUrlPath env url
    in
    case String.startsWith "devices" normalizedUrl of
        True ->
            deviceRouting env normalizedUrl

        False ->
            case ( normalizedUrl, maybeId ) of
                ( "register-device", Just _ ) ->
                    let
                        parser =
                            UrlParser.query targetDeviceIdQueryParser

                        -- Parse the quey, but pretend we're at the root, no matter where we are. This
                        -- is a workaround to avoid having to deal with the path we're actually hosted
                        -- under
                        parsedQuery =
                            UrlParser.parse parser { url | path = "" }

                        initialModel =
                            case parsedQuery of
                                Just (Just id) ->
                                    Route.DeviceRegistration.withInitialId id

                                _ ->
                                    Route.DeviceRegistration.default
                    in
                    Matched ( Just (DeviceRegistration initialModel), Cmd.none )

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
    Environment.getLoadedId env
        |> Maybe.map (routeLoadedEnv env url)
        |> Maybe.withDefault (Matched ( Nothing, Cmd.none ))


targetDeviceIdQueryParser : QueryParser.Parser (Maybe String)
targetDeviceIdQueryParser =
    QueryParser.string "device_target_id"


renderLogin : Environment.Environment -> Html.Html Message
renderLogin env =
    let
        loginContentText =
            Maybe.withDefault "" (Environment.getLocalizedContent env "login_page")

        loginContentDom =
            Result.withDefault [] (Result.map HTP.toVirtualDom (Html.Parser.run loginContentText))
    in
    Html.div [ Html.Attributes.class "lg:flex items-start h-full w-full relative" ]
        [ Html.div [ Html.Attributes.class "lg:flex-1 lg:pl-3" ]
            [ Html.div []
                [ Html.a
                    [ Html.Attributes.href env.configuration.loginUrl
                    , Html.Attributes.rel "noopener"
                    , Html.Attributes.target "_self"
                    ]
                    [ Html.text "Login" ]
                ]
            ]
        , Html.div [ Html.Attributes.class "lg:flex-1 lg:pr-3 flex-1 flex flex-col h-full relative" ]
            loginContentDom
        ]
