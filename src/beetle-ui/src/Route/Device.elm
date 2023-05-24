module Route.Device exposing (Message(..), Model, default, subscriptions, update, view)

import Button
import Environment
import Html
import Html.Attributes as ATT
import Html.Events as EV
import Http
import Icon
import Json.Decode
import Json.Encode as Encode
import Task
import Time
import TimeDiff


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
    | ToggleLights Bool
    | UpdateInput InputKinds
    | LoadedTime Time.Posix


type InputKinds
    = Message String
    | Link String


type QueuePayloadKinds
    = MessagePayload String
    | LinkPayload String
    | LightPayload Bool


type alias Model =
    { id : String
    , activeInput : ( InputKinds, Maybe (Maybe (Result Http.Error String)) )
    , loadedDevice : Maybe (Result Http.Error DeviceInfoResponse)
    , pendingRefresh : Maybe (Maybe (Result Http.Error DeviceInfoResponse))
    , pendingMessageJobs : List String
    , currentTime : Maybe Time.Posix
    }


subscriptions : Model -> Sub Message
subscriptions model =
    Time.every 2000 Tick


isBusy : Model -> Bool
isBusy model =
    let
        isSending =
            Tuple.second model.activeInput |> Maybe.map (always True) |> Maybe.withDefault False

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
    let
        isDisabled =
            isBusy model

        ( inputNode, inputToggles ) =
            case ( model.activeInput, isDisabled ) of
                ( _, True ) ->
                    ( Html.input [ EV.onInput SetMessage, ATT.value "", ATT.disabled isDisabled ]
                        []
                    , [ Button.view (Button.DisabledIcon Icon.Link)
                      , Html.div [ ATT.class "ml-2" ]
                            [ Button.view (Button.DisabledIcon Icon.File) ]
                      ]
                    )

                ( ( Link current, _ ), _ ) ->
                    ( Html.input [ EV.onInput SetMessage, ATT.value current, ATT.disabled isDisabled ]
                        []
                    , [ Button.view (Button.DisabledIcon Icon.Link)
                      , Html.div [ ATT.class "ml-2" ]
                            [ Button.view (Button.PrimaryIcon Icon.File (UpdateInput (Message ""))) ]
                      ]
                    )

                ( ( Message current, _ ), _ ) ->
                    ( Html.input [ EV.onInput SetMessage, ATT.value current, ATT.disabled isDisabled ]
                        []
                    , [ Button.view (Button.PrimaryIcon Icon.Link (UpdateInput (Link "")))
                      , Html.div [ ATT.class "ml-2" ]
                            [ Button.view (Button.DisabledIcon Icon.File) ]
                      ]
                    )

        sendButton =
            if isBusy model then
                Button.DisabledIcon Icon.Send

            else
                Button.PrimaryIcon Icon.Send AttemptMessage

        lightButtons =
            case isBusy model of
                True ->
                    [ Button.view (Button.DisabledIcon Icon.Sun)
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.DisabledIcon Icon.Moon) ]
                    ]

                False ->
                    [ Button.view (Button.SecondaryIcon Icon.Sun (ToggleLights True))
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.SecondaryIcon Icon.Moon (ToggleLights False)) ]
                    ]
    in
    Html.div [ ATT.class "px-4 py-3" ]
        [ Html.div [ ATT.class "pb-1 mb-1 flex items-center" ]
            [ Html.div [] [ Html.h2 [] [ Html.text model.id ] ]
            , Html.div [ ATT.class "lg:hidden flex ml-auto items-center" ] inputToggles
            ]
        , Html.div [ ATT.class "flex items-center" ]
            [ inputNode
            , Html.div [ ATT.class "ml-2" ] [ Button.view sendButton ]
            , Html.div [ ATT.class "hidden lg:flex ml-8 items-center" ] inputToggles
            ]
        , case model.loadedDevice of
            Nothing ->
                Html.div [ ATT.class "mt-2 pt-2" ] [ Html.text "Loading ..." ]

            Just (Err error) ->
                let
                    failureString =
                        case error of
                            Http.BadStatus _ ->
                                "Unknown Device"

                            _ ->
                                "Failed"
                in
                Html.div [ ATT.class "mt-2 pt-2" ] [ Html.text failureString ]

            Just (Ok info) ->
                Html.div []
                    [ Html.div [ ATT.class "flex items-center mt-2 justify-center" ] lightButtons
                    , deviceInfoTable model info
                    ]
        ]


deviceInfoTable : Model -> DeviceInfoResponse -> Html.Html Message
deviceInfoTable model info =
    let
        sentMessageCount =
            Maybe.withDefault 0 info.sent_message_count |> String.fromInt

        lastSeenText =
            case model.currentTime of
                Just time ->
                    let
                        theDiff =
                            TimeDiff.diff time (Time.millisToPosix info.last_seen)
                    in
                    Html.text (TimeDiff.toString theDiff)

                Nothing ->
                    Html.text (formatDeviceTime info.last_seen ++ "UTC")
    in
    Html.table [ ATT.class "w-full mt-2" ]
        [ Html.thead [] []
        , Html.tbody []
            [ Html.tr []
                [ Html.td [] [ Html.text "Total Messages Sent" ]
                , Html.td [] [ Html.text sentMessageCount ]
                ]
            , Html.tr []
                [ Html.td [] [ Html.text "Current Queue" ]
                , Html.td [] [ Html.text (String.fromInt info.current_queue_count) ]
                ]
            , Html.tr []
                [ Html.td [] [ Html.text "Last Seen" ]
                , Html.td [] [ lastSeenText ]
                ]
            , Html.tr []
                [ Html.td [] [ Html.text "First Seen" ]
                , Html.td [] [ Html.text (formatDeviceTime info.first_seen ++ "UTC") ]
                ]
            ]
        ]


queuedMessageDecoder : Json.Decode.Decoder String
queuedMessageDecoder =
    Json.Decode.field "id" Json.Decode.string


postMessage : Environment.Environment -> String -> QueuePayloadKinds -> Cmd Message
postMessage env id payloadKind =
    let
        payload =
            case payloadKind of
                LightPayload isOn ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "lights" )
                                    , ( "beetle:content", Encode.bool isOn )
                                    ]
                              )
                            ]
                        )

                LinkPayload str ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "link" )
                                    , ( "beetle:content", Encode.string str )
                                    ]
                              )
                            ]
                        )

                MessagePayload str ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "message" )
                                    , ( "beetle:content", Encode.string str )
                                    ]
                              )
                            ]
                        )
    in
    Http.post
        { url = Environment.apiRoute env "device-queue"
        , body = payload
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
        Tick time ->
            let
                ( command, pendingRefresh ) =
                    case model.pendingRefresh of
                        Just Nothing ->
                            ( Cmd.none, model.pendingRefresh )

                        Nothing ->
                            ( fetchDevice env model.id, Just Nothing )

                        Just (Just _) ->
                            ( fetchDevice env model.id, Just Nothing )
            in
            ( { model | currentTime = Just time, pendingRefresh = pendingRefresh }, command )

        UpdateInput newInput ->
            ( { model | activeInput = ( newInput, Tuple.second model.activeInput ) }, Cmd.none )

        SetMessage messageText ->
            let
                nextInput =
                    case Tuple.first model.activeInput of
                        Message _ ->
                            Message messageText

                        Link _ ->
                            Link messageText
            in
            -- ( setMessage model messageText, Cmd.none )
            ( { model | activeInput = ( nextInput, Tuple.second model.activeInput ) }, Cmd.none )

        LoadedDeviceInfo infoResult ->
            let
                pendingRefresh =
                    Maybe.map (always (Just infoResult)) model.pendingRefresh
            in
            ( { model | pendingRefresh = pendingRefresh, loadedDevice = Just infoResult }, Cmd.none )

        Loaded _ ->
            let
                emptiedInput =
                    case Tuple.first model.activeInput of
                        Message _ ->
                            Message ""

                        Link _ ->
                            Link ""
            in
            ( { model | activeInput = ( emptiedInput, Nothing ) }, Cmd.none )

        ToggleLights state ->
            ( { model | activeInput = ( Tuple.first model.activeInput, Just Nothing ) }
            , postMessage env model.id (LightPayload state)
            )

        QueuedMessageJob (Ok jobId) ->
            ( { model | pendingMessageJobs = jobId :: model.pendingMessageJobs }, Cmd.none )

        QueuedMessageJob (Err _) ->
            ( model, Cmd.none )

        AttemptMessage ->
            let
                payload =
                    case Tuple.first model.activeInput of
                        Message str ->
                            MessagePayload str

                        Link str ->
                            LinkPayload str

                newInput =
                    ( Tuple.first model.activeInput, Just Nothing )
            in
            ( { model | activeInput = newInput }, postMessage env model.id payload )

        LoadedTime now ->
            ( { model | currentTime = Just now }, Cmd.none )


getNow : Cmd Message
getNow =
    Task.perform LoadedTime Time.now


default : Environment.Environment -> String -> ( Model, Cmd Message )
default env id =
    ( { id = id
      , activeInput = ( Message "", Nothing )
      , loadedDevice = Nothing
      , pendingMessageJobs = []
      , pendingRefresh = Nothing
      , currentTime = Nothing
      }
    , Cmd.batch [ fetchDevice env id, getNow ]
    )
