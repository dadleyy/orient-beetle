module Main exposing (..)

import Browser
import Browser.Navigation as Nav
import Html
import Html.Attributes
import Http
import Json.Decode
import Url



-- MAIN


main : Program Flags Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        , onUrlChange = UrlChanged
        , onUrlRequest = LinkClicked
        }



-- MODEL


type alias Model =
    { key : Nav.Key
    , url : Url.Url
    , flags : Flags
    , status : Maybe StatusResponse
    }


type alias Flags =
    { api : String, root : String, version : String, loginUrl : String }


defaultModel : Flags -> Url.Url -> Nav.Key -> Model
defaultModel flags url key =
    { key = key, url = url, flags = flags, status = Nothing }


fetchStatus : Flags -> Cmd Msg
fetchStatus flags =
    Http.get { url = String.concat [ flags.api, "status" ], expect = Http.expectJson StatusFetch statusDecoder }


init : Flags -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
init flags url key =
    ( defaultModel flags url key, fetchStatus flags )



-- UPDATE


type Msg
    = LinkClicked Browser.UrlRequest
    | UrlChanged Url.Url
    | StatusFetch (Result Http.Error StatusResponse)


type alias StatusResponse =
    { version : String
    , timestamp : String
    }


statusDecoder : Json.Decode.Decoder StatusResponse
statusDecoder =
    Json.Decode.map2 StatusResponse
        (Json.Decode.field "version" Json.Decode.string)
        (Json.Decode.field "timestamp" Json.Decode.string)


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        StatusFetch res ->
            case res of
                Ok data ->
                    ( { model | status = Just data }, Cmd.none )

                Err _ ->
                    ( model, Cmd.batch [] )

        LinkClicked urlRequest ->
            case urlRequest of
                Browser.Internal url ->
                    ( model, Nav.pushUrl model.key (Url.toString url) )

                Browser.External href ->
                    ( model, Nav.load href )

        UrlChanged url ->
            ( { model | url = url }
            , Cmd.none
            )



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none



-- VIEW


header : Model -> Html.Html Msg
header model =
    case model.status of
        Nothing ->
            Html.div [ Html.Attributes.class "cont-dark px-4 py-3" ] []

        Just data ->
            Html.div [ Html.Attributes.class "cont-dark px-4 py-3" ] []


buildRoutePath : Model -> String -> String
buildRoutePath model path =
    String.concat [ model.flags.root, path ]


body : Model -> Html.Html Msg
body model =
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.text "The current URL is: "
        , Html.i [] [ Html.text (Url.toString model.url) ]
        , Html.ul []
            [ viewLink (buildRoutePath model "home")
            , viewLink (buildRoutePath model "profile")
            ]
        ]


externalLink : String -> String -> Html.Html Msg
externalLink addr text =
    Html.a [ Html.Attributes.href addr, Html.Attributes.rel "noopener", Html.Attributes.target "_blank" ] [ Html.text text ]


statusFooter : StatusResponse -> Html.Html Msg
statusFooter data =
    Html.div [] [ Html.text (String.concat [ String.slice 0 7 data.version, " @ ", data.timestamp ]) ]


footer : Model -> Html.Html Msg
footer model =
    Html.div [ Html.Attributes.class "cont-dark px-4 py-2 flex" ]
        [ Html.div [] [ externalLink "https://github.com/dadleyy/orient-beetle" "github" ]
        , Html.div [ Html.Attributes.class "ml-auto" ]
            [ Maybe.withDefault (Html.div [] []) (Maybe.map statusFooter model.status)
            ]
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
