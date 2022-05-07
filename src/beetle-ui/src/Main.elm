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
    { api : String, root : String, version : String }


init : Flags -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
init flags url key =
    ( Model key url flags Nothing, Http.get { url = String.concat [ flags.api, "status" ], expect = Http.expectJson StatusFetch statusDecoder } )



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

                Err error ->
                    Debug.log (Debug.toString error)
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
            Html.div [ Html.Attributes.class "pending" ] []

        Just data ->
            Html.div [ Html.Attributes.class "loaded" ] [ Html.text (String.concat [ data.version, " @ ", data.timestamp ]) ]


view : Model -> Browser.Document Msg
view model =
    { title = "beetle-ui"
    , body =
        [ header model
        , Html.div []
            [ Html.text "The current URL is: "
            , Html.i [] [ Html.text (Url.toString model.url) ]
            , Html.ul []
                [ viewLink "/home"
                , viewLink "/profile"
                , viewLink "/reviews/the-century-of-the-self"
                , viewLink "/reviews/public-opinion"
                , viewLink "/reviews/shah-of-shahs"
                ]
            ]
        ]
    }


viewLink : String -> Html.Html msg
viewLink path =
    Html.li [] [ Html.a [ Html.Attributes.href path ] [ Html.text path ] ]
