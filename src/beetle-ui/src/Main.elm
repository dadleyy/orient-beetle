module Main exposing (..)

import Browser
import Browser.Navigation as Nav
import Environment
import Html
import Html.Attributes
import Http
import Json.Decode
import Route
import Url


main : Program Environment.Configuration Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        , onUrlChange = UrlChanged
        , onUrlRequest = LinkClicked
        }


type Route
    = Login
    | Home


type alias Model =
    { key : Nav.Key
    , url : Url.Url
    , env : Environment.Environment
    , route : Maybe Route.Route
    }


type Msg
    = LinkClicked Browser.UrlRequest
    | UrlChanged Url.Url
    | EnvironmentMessage Environment.Message
    | RouteMessage Route.Message


defaultModel : Environment.Configuration -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
defaultModel flags url key =
    let
        env =
            Environment.default flags

        ( route, loader ) =
            case Route.fromUrl env url of
                Route.Matched inner ->
                    ( Tuple.first inner, Tuple.second inner |> Cmd.map RouteMessage )

                Route.Redirect dest ->
                    ( Nothing, Nav.pushUrl key dest )
    in
    ( { route = route, key = key, url = url, env = env }, loader )


init : Environment.Configuration -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
init flags url key =
    let
        ( model, cmd ) =
            defaultModel flags url key
    in
    ( model, Cmd.batch [ initEnv model.env, cmd ] )


initEnv : Environment.Environment -> Cmd Msg
initEnv env =
    Environment.boot env |> Cmd.map EnvironmentMessage


update : Msg -> Model -> ( Model, Cmd Msg )
update message model =
    case ( message, model.route ) of
        -- Links are not really specific to any given route/model state.
        ( LinkClicked urlRequest, _ ) ->
            case urlRequest of
                Browser.Internal url ->
                    ( model, Nav.pushUrl model.key (Url.toString url) )

                Browser.External href ->
                    ( model, Nav.load href )

        -- Environment messages are also not specific to a route/model as they
        -- will probably come in early
        ( EnvironmentMessage em, _ ) ->
            let
                updated =
                    Environment.update em model.env

                ( newRoute, cmd ) =
                    case Route.fromUrl updated model.url of
                        Route.Matched inner ->
                            inner

                        Route.Redirect dest ->
                            ( Nothing, Nav.pushUrl model.key dest )
            in
            ( { model | env = updated, route = newRoute }, cmd |> Cmd.map RouteMessage )

        -- The url change here is where we do all of our route transition magic,
        -- where the route module delegates an intial load and stuff to sub
        -- modules.
        ( UrlChanged url, _ ) ->
            let
                ( next, cmd ) =
                    case Route.fromUrl model.env url of
                        Route.Matched inner ->
                            ( Tuple.first inner, Tuple.second inner |> Cmd.map RouteMessage )

                        Route.Redirect redir ->
                            ( Nothing, Nav.pushUrl model.key redir )
            in
            ( { model | url = url, route = next }, cmd )

        -- If we don't have a current route and receive some route-specific message,
        -- do nothing.
        ( RouteMessage _, Nothing ) ->
            ( model, Cmd.none )

        -- Handle login route messages
        ( RouteMessage inner, Just route ) ->
            let
                ( next, cmd ) =
                    Route.update model.env inner route
            in
            ( { model | route = Just next }, cmd |> Cmd.map RouteMessage )


subscriptions : Model -> Sub Msg
subscriptions model =
    Maybe.withDefault Sub.none (Maybe.map (Sub.map RouteMessage) (Maybe.map Route.subscriptions model.route))


header : Model -> Html.Html Msg
header model =
    case Environment.getId model.env of
        Nothing ->
            Html.div [ Html.Attributes.class "cont-dark px-4 py-3" ] []

        Just id ->
            Html.div [ Html.Attributes.class "cont-dark px-4 py-3 flex items-center" ]
                [ Html.div [ Html.Attributes.class "truncate" ] [ Html.text (String.concat [ "oid: ", id ]) ]
                , Html.div [ Html.Attributes.class "ml-auto" ]
                    [ Html.a
                        [ Html.Attributes.href (Environment.buildRoutePath model.env "home") ]
                        [ Html.text "home" ]
                    ]
                ]


body : Model -> Html.Html Msg
body model =
    case ( Environment.getId model.env, model.route ) of
        ( Nothing, Just Route.Login ) ->
            Html.div
                [ Html.Attributes.class "flex-1 main" ]
                [ Route.view model.env Route.Login |> Html.map RouteMessage ]

        ( Just _, Just route ) ->
            Html.div
                [ Html.Attributes.class "flex-1 main" ]
                [ Route.view model.env route |> Html.map RouteMessage ]

        -- If we have a session but not a route, link back to home.
        ( Just _, Nothing ) ->
            Html.div
                [ Html.Attributes.class "flex-1 px-4 py-3 main" ]
                [ Html.a
                    [ Html.Attributes.href (Environment.buildRoutePath model.env "home") ]
                    [ Html.text "home" ]
                ]

        -- Only our login route should ever be dealing with non-loaded sessions
        ( Nothing, _ ) ->
            Html.div [ Html.Attributes.class "flex-1 px-4 py-3 main" ] [ Html.text "loading..." ]


externalLink : String -> String -> Html.Html Msg
externalLink addr text =
    Html.a
        [ Html.Attributes.href addr, Html.Attributes.rel "noopener", Html.Attributes.target "_blank" ]
        [ Html.text text ]


footer : Model -> Html.Html Msg
footer model =
    Html.div [ Html.Attributes.class "cont-dark px-4 py-2 flex" ]
        [ Html.div [] [ externalLink "https://github.com/dadleyy/orient-beetle" "github" ]
        , Html.div [ Html.Attributes.class "ml-auto truncate" ]
            [ Environment.statusFooter model.env |> Html.map EnvironmentMessage ]
        ]


view : Model -> Browser.Document Msg
view model =
    { title = "beetle-ui"
    , body =
        [ Html.div [ Html.Attributes.class "flex flex-col relative h-full w-full" ]
            [ header model
            , body model
            , footer model
            ]
        ]
    }


viewLink : String -> Html.Html msg
viewLink path =
    Html.li [] [ Html.a [ Html.Attributes.href path ] [ Html.text path ] ]
