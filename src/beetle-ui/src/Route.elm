module Route exposing (Message(..), Route(..), RouteInitialization(..), fromUrl, subscriptions, update, view)

import Environment
import Html
import Html.Attributes
import Html.Parser
import Html.Parser.Util as HTP
import Route.Account
import Route.Device
import Route.DeviceRegistration
import Route.Home
import Url
import Url.Parser as UrlParser exposing ((</>))
import Url.Parser.Query as QueryParser



-- When trying to initialize a route based on a url, we will either find a route and initialize it to
-- its default state, or have some redirect to another url.


type RouteInitialization
    = Matched ( Maybe Route, Cmd Message )
    | Redirect String


type Route
    = Login
    | Account Route.Account.Model
    | Home Route.Home.Model
    | Device Route.Device.Model
    | DeviceRegistration Route.DeviceRegistration.Model


type RouteUrl
    = AccountUrl
    | DeviceHomeUrl String
    | HomeUrl
    | LoginUrl
    | DeviceRegistrationUrl


type Message
    = HomeMessage Route.Home.Message
    | DeviceMessage Route.Device.Message
    | DeviceRegistrationMessage Route.DeviceRegistration.Message
    | AccountMessage Route.Account.Message


subscriptions : Route -> Sub Message
subscriptions route =
    case route of
        DeviceRegistration regModel ->
            Sub.map DeviceRegistrationMessage (Route.DeviceRegistration.subscriptions regModel)

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

        Account inner ->
            Route.Account.view inner env |> Html.map AccountMessage

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


routeParser : UrlParser.Parser (RouteUrl -> a) a
routeParser =
    UrlParser.oneOf
        [ UrlParser.map AccountUrl (UrlParser.s "account" </> UrlParser.top)
        , UrlParser.map DeviceHomeUrl (UrlParser.s "devices" </> UrlParser.string)
        , UrlParser.map HomeUrl (UrlParser.s "home" </> UrlParser.top)
        , UrlParser.map LoginUrl (UrlParser.s "login" </> UrlParser.top)
        , UrlParser.map DeviceRegistrationUrl (UrlParser.s "register-device" </> UrlParser.top)
        ]


trimLeftMatches : String -> String -> String
trimLeftMatches predicate input =
    if String.startsWith predicate input then
        String.dropLeft (String.length predicate) input

    else
        input



-- TODO: update all routing to use canonical methods provided by elm. Some of the following
--       was implemented using a nasty handrolled version.


routeLoadedEnv : Environment.Environment -> Url.Url -> Maybe String -> RouteInitialization
routeLoadedEnv env url maybeId =
    let
        normalizedUrl =
            Environment.normalizeUrlPath env url

        normalizedUrlPathing =
            { url | path = trimLeftMatches env.configuration.root url.path }

        parsedUrl =
            UrlParser.parse routeParser normalizedUrlPathing

        homeRedirect =
            Redirect (Environment.buildRoutePath env "home")
    in
    case parsedUrl of
        Just AccountUrl ->
            let
                ( maybeAccountModel, cmd ) =
                    Route.Account.default env
            in
            maybeAccountModel
                |> Maybe.map (\m -> Matched ( Just (Account m), cmd |> Cmd.map AccountMessage ))
                |> Maybe.withDefault homeRedirect

        Just HomeUrl ->
            let
                ( route, cmd ) =
                    Route.Home.default env

                ifLoaded =
                    Matched ( Just (Home route), cmd |> Cmd.map HomeMessage )
            in
            Maybe.map (always ifLoaded) maybeId
                |> Maybe.withDefault (Redirect (Environment.buildRoutePath env "login"))

        Just (DeviceHomeUrl id) ->
            case Environment.getId env of
                Just _ ->
                    let
                        ( model, cmd ) =
                            Route.Device.default env id
                    in
                    Matched ( Just (Device model), cmd |> Cmd.map DeviceMessage )

                Nothing ->
                    Redirect (Environment.buildRoutePath env "login")

        Just DeviceRegistrationUrl ->
            let
                parser =
                    UrlParser.query targetDeviceIdQueryParser

                -- Parse the quey, but pretend we're at the root, no matter where we are. This
                -- is a workaround to avoid having to deal with the path we're actually hosted
                -- under.
                parsedQuery =
                    UrlParser.parse parser { url | path = "" }

                initialModel =
                    case parsedQuery of
                        Just (Just id) ->
                            Route.DeviceRegistration.withInitialId id

                        _ ->
                            Route.DeviceRegistration.default

                ifLoaded =
                    Matched ( Just (DeviceRegistration initialModel), Cmd.none )
            in
            Maybe.map (always ifLoaded) maybeId
                |> Maybe.withDefault (Redirect (Environment.buildRoutePath env "login"))

        Just LoginUrl ->
            let
                ifLoaded =
                    Redirect (Environment.buildRoutePath env "home")
            in
            Maybe.map (always ifLoaded) maybeId
                |> Maybe.withDefault (Matched ( Just Login, Cmd.none ))

        Nothing ->
            Redirect (Environment.buildRoutePath env "home")


fromUrl : Environment.Environment -> Url.Url -> RouteInitialization
fromUrl env url =
    -- This is a maybe of a maybe. We don't want to redirect if there is no session _yet_.
    let
        maybeLoadedId =
            Environment.getLoadedId env
    in
    Maybe.map (routeLoadedEnv env url) maybeLoadedId
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
