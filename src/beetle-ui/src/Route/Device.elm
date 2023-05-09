module Route.Device exposing (Message(..), Model, default, subscriptions, update, view)

import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Json.Decode
import Json.Encode
import Time


type alias DeviceInfoResponse =
    { id : String
    , last_seen : Int
    , first_seen : Int
    , sent_message_count : Maybe Int
    , current_queue_count : Int
    }


type Message
    = Loaded (Result Http.Error ())
    | LoadedDeviceInfo (Result Http.Error DeviceInfoResponse)
    | QueuedMessageJob (Result Http.Error String)
    | Tick Time.Posix
    | AttemptMessage
    | SetMessage String


type alias Model =
    { id : String
    , newMessage : ( String, Maybe (Maybe String) )
    , loadedDevice : Maybe (Result Http.Error DeviceInfoResponse)
    , pendingMessageJobs : List String
    }


setMessage : Model -> String -> Model
setMessage model message =
    { model | newMessage = ( message, Nothing ) }


subscriptions : Model -> Sub Message
subscriptions model =
    Time.every 2000 Tick


getMessage : Model -> String
getMessage model =
    Tuple.first model.newMessage


isBusy : Model -> Bool
isBusy model =
    let
        isSending =
            Tuple.second model.newMessage |> Maybe.map (always True) |> Maybe.withDefault False

        isLoading =
            case model.loadedDevice of
                Just (Ok _) ->
                    False

                _ ->
                    True
    in
    isSending || isLoading


formatDeviceMonth : Time.Month -> String
formatDeviceMonth month =
    case month of
        Time.Jan ->
            "01"

        Time.Feb ->
            "02"

        Time.Mar ->
            "03"

        Time.Apr ->
            "04"

        Time.May ->
            "05"

        Time.Jun ->
            "06"

        Time.Jul ->
            "07"

        Time.Aug ->
            "08"

        Time.Sep ->
            "09"

        Time.Oct ->
            "10"

        Time.Nov ->
            "11"

        Time.Dec ->
            "12"


formatDeviceTime : Int -> String
formatDeviceTime time =
    let
        posixValue =
            Time.millisToPosix time
    in
    String.join "/"
        [ String.fromInt (Time.toYear Time.utc posixValue)
        , formatDeviceMonth (Time.toMonth Time.utc posixValue)
        , String.fromInt (Time.toDay Time.utc posixValue)
        ]
        ++ " "
        ++ String.join ":"
            [ String.padLeft 2 '0' (String.fromInt (Time.toHour Time.utc posixValue))
            , String.padLeft 2 '0' (String.fromInt (Time.toMinute Time.utc posixValue))
            , String.padLeft 2 '0' (String.fromInt (Time.toSecond Time.utc posixValue))
            ]


view : Model -> Environment.Environment -> Html.Html Message
view model env =
    Html.div [ Html.Attributes.class "px-4 py-3" ]
        [ Html.div [ Html.Attributes.class "pb-1 mb-1" ]
            [ Html.h2 []
                [ Html.text model.id ]
            ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input
                [ Html.Events.onInput SetMessage
                , Html.Attributes.value (getMessage model)
                , Html.Attributes.disabled (isBusy model)
                ]
                []
            , Html.button [ Html.Events.onClick AttemptMessage, Html.Attributes.disabled (isBusy model) ]
                [ Html.text "send" ]
            ]
        , case model.loadedDevice of
            Nothing ->
                Html.div [ Html.Attributes.class "mt-2 pt-2" ] [ Html.text "Loading ..." ]

            Just (Err error) ->
                let
                    failureString =
                        case error of
                            Http.BadStatus _ ->
                                "Unknown Device"

                            _ ->
                                "Failed"
                in
                Html.div [ Html.Attributes.class "mt-2 pt-2" ] [ Html.text failureString ]

            Just (Ok info) ->
                let
                    sent_message_count =
                        Maybe.withDefault 0 info.sent_message_count |> String.fromInt
                in
                Html.div [ Html.Attributes.class "mt-2 pt-2" ]
                    [ Html.div [] [ Html.code [] [ Html.text ("total messages sent: " ++ sent_message_count) ] ]
                    , Html.div [] [ Html.code [] [ Html.text ("current messages queued: " ++ String.fromInt info.current_queue_count) ] ]
                    , Html.div [] [ Html.code [] [ Html.text ("last seen: " ++ formatDeviceTime info.last_seen ++ "UTC") ] ]
                    , Html.div [] [ Html.code [] [ Html.text ("first seen: " ++ formatDeviceTime info.first_seen ++ "UTC") ] ]
                    ]
        ]


queuedMessageDecoder : Json.Decode.Decoder String
queuedMessageDecoder =
    Json.Decode.field "id" Json.Decode.string


postMessage : Environment.Environment -> Model -> Cmd Message
postMessage env model =
    Http.post
        { url = Environment.apiRoute env "device-message"
        , body =
            Http.jsonBody
                (Json.Encode.object
                    [ ( "device_id", Json.Encode.string model.id )
                    , ( "message", Json.Encode.string (getMessage model) )
                    ]
                )
        , expect = Http.expectWhatever Loaded
        }


infoDecoder : Json.Decode.Decoder DeviceInfoResponse
infoDecoder =
    Json.Decode.map5 DeviceInfoResponse
        (Json.Decode.field "id" Json.Decode.string)
        (Json.Decode.field "last_seen" Json.Decode.int)
        (Json.Decode.field "first_seen" Json.Decode.int)
        (Json.Decode.field "sent_message_count" (Json.Decode.maybe Json.Decode.int))
        (Json.Decode.field "current_queue_count" Json.Decode.int)


fetchDevice : Environment.Environment -> String -> Cmd Message
fetchDevice env id =
    Http.get
        { url = Environment.apiRoute env ("device-info?id=" ++ id)
        , expect = Http.expectJson LoadedDeviceInfo infoDecoder
        }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        Tick _ ->
            ( model, fetchDevice env model.id )

        SetMessage messageText ->
            ( setMessage model messageText, Cmd.none )

        LoadedDeviceInfo infoResult ->
            ( { model | loadedDevice = Just infoResult }, Cmd.none )

        Loaded _ ->
            ( { model | newMessage = ( "", Nothing ) }, Cmd.none )

        QueuedMessageJob (Ok jobId) ->
            ( { model
                | newMessage = ( "", Nothing )
                , pendingMessageJobs = jobId :: model.pendingMessageJobs
              }
            , Cmd.none
            )

        QueuedMessageJob (Err _) ->
            ( { model | newMessage = ( "", Nothing ) }, Cmd.none )

        AttemptMessage ->
            ( { model | newMessage = ( Tuple.first model.newMessage, Just Nothing ) }, postMessage env model )


default : Environment.Environment -> String -> ( Model, Cmd Message )
default env id =
    ( { id = id
      , newMessage = ( "", Nothing )
      , loadedDevice = Nothing
      , pendingMessageJobs = []
      }
    , Cmd.batch [ fetchDevice env id ]
    )
