module Route.DeviceRegistration exposing
    ( Message(..)
    , Model
    , default
    , subscriptions
    , update
    , view
    , withInitialId
    )

import Browser.Navigation as Nav
import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Job
import Json.Decode as Decode
import Json.Encode as Encode
import Time


type alias RegistrationResponse =
    { id : String }


type JobPollingState
    = WaitingForId
    | PollingId String
    | PolledId String


type alias Model =
    { newDevice : String
    , pendingAttempt : Maybe JobPollingState
    , loadingJob : Bool
    , alert : Maybe Alert
    }


type Message
    = SetNewDeviceId String
    | AttemptDeviceClaim
    | RegisteredDevice (Result Http.Error RegistrationResponse)
    | Tick Time.Posix
    | LoadedJob (Result Http.Error Job.Job)


type Alert
    = Warning String
    | Happy String


default : Model
default =
    { newDevice = "", alert = Nothing, pendingAttempt = Nothing, loadingJob = False }


withInitialId : String -> Model
withInitialId id =
    { default | newDevice = id }


loadPendingJob : Environment.Environment -> String -> Cmd Message
loadPendingJob env jobId =
    let
        url =
            Environment.apiRoute env "jobs" ++ "?id=" ++ jobId
    in
    Http.get
        { url = url
        , expect = Http.expectJson LoadedJob Job.decoder
        }


finishPollAttempt : JobPollingState -> JobPollingState
finishPollAttempt state =
    case state of
        PolledId id ->
            PollingId id

        _ ->
            state


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        Tick time ->
            let
                ( pendingAttempt, fetchCmd ) =
                    case model.pendingAttempt of
                        Just WaitingForId ->
                            ( Just WaitingForId, Cmd.none )

                        Just (PollingId id) ->
                            ( Just (PolledId id), loadPendingJob env id )

                        Just (PolledId id) ->
                            ( Just (PolledId id), Cmd.none )

                        Nothing ->
                            ( Nothing, Cmd.none )
            in
            ( { model | pendingAttempt = pendingAttempt }, fetchCmd )

        LoadedJob loadResult ->
            let
                mappedResult =
                    Result.map Job.asResult loadResult

                ( alert, pendingAttempt, cmd ) =
                    case mappedResult of
                        Err err ->
                            ( Just (Warning "Failed"), Nothing, Cmd.none )

                        Ok Job.Pending ->
                            ( Nothing, Maybe.map finishPollAttempt model.pendingAttempt, Cmd.none )

                        Ok Job.Success ->
                            let
                                redir =
                                    Nav.pushUrl env.navKey ("/devices/" ++ model.newDevice)
                            in
                            ( Nothing, Nothing, redir )

                        Ok (Job.Failed reason) ->
                            ( Just (Warning reason), Nothing, Cmd.none )

                        Ok Job.Unknown ->
                            ( Just (Warning "Unknown job result"), Nothing, Cmd.none )
            in
            ( { model | loadingJob = False, pendingAttempt = pendingAttempt, alert = alert }, cmd )

        SetNewDeviceId id ->
            ( { model | newDevice = id }, Cmd.none )

        AttemptDeviceClaim ->
            ( { model | pendingAttempt = Just WaitingForId }, addDevice env model.newDevice )

        RegisteredDevice result ->
            case result of
                Ok registrationRes ->
                    ( { model | pendingAttempt = Just (PollingId registrationRes.id) }, Cmd.none )

                Err error ->
                    ( { model | newDevice = "", pendingAttempt = Nothing, alert = Just (Warning "Failed") }, Cmd.none )


view : Environment.Environment -> Model -> Html.Html Message
view env model =
    Html.div [ Html.Attributes.class "px-4 py-3" ] [ deviceRegistrationForm env model ]


hasPendingAddition : Model -> Bool
hasPendingAddition model =
    model.pendingAttempt |> Maybe.map (always True) |> Maybe.withDefault False


addDevice : Environment.Environment -> String -> Cmd Message
addDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Encode.object [ ( "device_id", Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


registrationDecoder : Decode.Decoder RegistrationResponse
registrationDecoder =
    Decode.map RegistrationResponse (Decode.field "id" Decode.string)


subscriptions : Model -> Sub Message
subscriptions model =
    case model.pendingAttempt of
        Just _ ->
            Time.every 2000 Tick

        Nothing ->
            Sub.none


deviceRegistrationForm : Environment.Environment -> Model -> Html.Html Message
deviceRegistrationForm env model =
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.div [ Html.Attributes.class "pb-3 py-2" ] [ Html.b [] [ Html.text "Add Device" ] ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input
                [ Html.Attributes.placeholder "device id"
                , Html.Attributes.value model.newDevice
                , Html.Attributes.class "block mr-2"
                , Html.Attributes.disabled (hasPendingAddition model)
                , Html.Events.onInput SetNewDeviceId
                ]
                []
            , Html.button
                [ Html.Attributes.disabled (hasPendingAddition model || String.isEmpty model.newDevice)
                , Html.Events.onClick AttemptDeviceClaim
                ]
                [ Html.text "Add" ]
            ]
        , case model.alert of
            Nothing ->
                Html.div [] []

            Just (Happy text) ->
                Html.div [ Html.Attributes.class "mt-2 pill happy" ] [ Html.text text ]

            Just (Warning text) ->
                Html.div [ Html.Attributes.class "mt-2 pill sad" ] [ Html.text text ]
        ]
