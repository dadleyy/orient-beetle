module Route.Device exposing (Message(..), Model, default, subscriptions, update, view)

import Alert
import Button
import DeviceAuthority as DA
import DeviceQueue as DQ
import DeviceSchedule as DS
import Dropdown
import Environment
import Html
import Html.Attributes as ATT
import Html.Events as EV
import Http
import Icon
import Job
import Json.Decode as D
import Json.Encode as Encode
import Task
import Time
import TimeDiff


type alias DeviceInfoResponse =
    { id : String
    , lastSeen : Int
    , nickname : Maybe String
    , firstSeen : Int
    , sentMessageCount : Maybe Int
    , currentQueueCount : Int
    }


type SettingsMenuMessage
    = StartRename
    | QueueWelcomeScannable
    | MakePublic
    | MakePrivate


type QuickActions
    = RefreshRender
    | ClearRender


type Message
    = Loaded (Result Http.Error ())
    | LoadedDeviceAuthority (Result Http.Error DA.DeviceAuthorityResponse)
    | LoadedDeviceInfo (Result Http.Error DeviceInfoResponse)
    | QueuedMessageJob (Result Http.Error Job.JobHandle)
    | Tick Time.Posix
    | LoadedJobHandle Job.JobHandle (Result Http.Error Job.Job)
    | QuickAction QuickActions
    | AttemptMessage
    | SetMessage String
    | ToggleLights Bool
    | ToggleSchedule Bool
    | ClearAlert
    | UpdateInput InputKinds
    | SettingsMenuUpdate Dropdown.Dropdown (Maybe SettingsMenuMessage)
    | LoadedTime Time.Posix


type InputKinds
    = Message String
    | Link String
    | DeviceName String


type alias Model =
    { id : String
    , activeInput : ( InputKinds, Maybe (Maybe (Result Http.Error String)) )
    , alert : Maybe Alert.Alert
    , loadedDevice : Maybe (Result Http.Error DeviceInfoResponse)
    , loadedAuthority : Maybe (Result Http.Error DA.DeviceAuthorityResponse)
    , pendingRefresh : Maybe (Maybe (Result Http.Error DeviceInfoResponse))
    , pendingMessageJobs : List Job.JobHandle
    , currentTime : Maybe Time.Posix
    , settingsMenu : Dropdown.Dropdown
    }


subscriptions : Model -> Sub Message
subscriptions model =
    Sub.batch
        [ Time.every 2000 Tick
        , Dropdown.subscriptions SettingsMenuUpdate model.settingsMenu
        ]


isBusy : Model -> Bool
isBusy model =
    let
        isSending =
            Tuple.second model.activeInput |> Maybe.map (always True) |> Maybe.withDefault False

        isPolling =
            List.length model.pendingMessageJobs > 0

        isLoading =
            case model.loadedDevice of
                Just (Ok _) ->
                    False

                _ ->
                    True
    in
    isSending || isLoading || isPolling


resetInput : InputKinds -> InputKinds
resetInput kind =
    case kind of
        DeviceName _ ->
            Message ""

        Message _ ->
            Message ""

        Link _ ->
            Link ""


activeLinkToggles : List (Html.Html Message)
activeLinkToggles =
    [ Button.view (Button.DisabledIcon Icon.Link)
    , Html.div [ ATT.class "ml-2" ]
        [ Button.view (Button.PrimaryIcon Icon.File (UpdateInput (Message ""))) ]
    ]


activeMessageToggles : List (Html.Html Message)
activeMessageToggles =
    [ Button.view (Button.PrimaryIcon Icon.Link (UpdateInput (Link "")))
    , Html.div [ ATT.class "ml-2" ]
        [ Button.view (Button.DisabledIcon Icon.File) ]
    ]


disabledToggles : List (Html.Html Message)
disabledToggles =
    [ Button.view (Button.DisabledIcon Icon.Link)
    , Html.div [ ATT.class "ml-2" ]
        [ Button.view (Button.DisabledIcon Icon.File) ]
    ]


view : Model -> Environment.Environment -> Html.Html Message
view model env =
    let
        isDisabled =
            isBusy model

        activeInputTextbox str =
            Html.input [ EV.onInput SetMessage, ATT.value str, ATT.disabled isDisabled ] []

        ( inputNode, inputToggles ) =
            case ( model.activeInput, isDisabled ) of
                ( _, True ) ->
                    ( activeInputTextbox "", disabledToggles )

                ( ( DeviceName current, _ ), _ ) ->
                    let
                        back =
                            Button.view (Button.PrimaryIcon Icon.Cancel (UpdateInput (Message "")))

                        wrapped =
                            Html.div [ ATT.class "flex items-center flex-1" ]
                                [ Html.div [ ATT.class "mr-4" ] [ Html.text "Rename:" ]
                                , activeInputTextbox current
                                ]
                    in
                    ( wrapped, [ back ] )

                ( ( Link current, _ ), _ ) ->
                    ( activeInputTextbox current, activeLinkToggles )

                ( ( Message current, _ ), _ ) ->
                    ( activeInputTextbox current, activeMessageToggles )

        sendButton =
            if isBusy model then
                Button.DisabledIcon Icon.Send

            else
                Button.PrimaryIcon Icon.Send AttemptMessage

        settingsMenu =
            [ ( StartRename, Html.div [] [ Html.text "Rename Device" ] )
            , ( QueueWelcomeScannable, Html.div [] [ Html.text "Send Registration Scannable" ] )
            , ( MakePublic, Html.div [] [ Html.text "Make Public" ] )
            , ( MakePrivate, Html.div [] [ Html.text "Make Private" ] )
            ]
    in
    Html.div [ ATT.class "px-4 py-3" ]
        [ viewAlert model
        , Html.div [ ATT.class "pb-1 mb-1 flex items-center" ]
            [ modelInfoHeader model
            , Html.div [ ATT.class "ml-auto" ]
                [ Dropdown.view model.settingsMenu SettingsMenuUpdate settingsMenu ]
            , Html.div [ ATT.class "lg:hidden flex ml-2 items-center" ] inputToggles
            ]
        , Html.div [ ATT.class "flex items-center" ]
            [ inputNode
            , Html.div [ ATT.class "ml-2" ] [ Button.view sendButton ]
            , Html.div [ ATT.class "hidden lg:flex ml-8 items-center" ] inputToggles
            ]
        , bottom model env
        ]


bottom : Model -> Environment.Environment -> Html.Html Message
bottom model env =
    let
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
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.SecondaryIcon Icon.CalendarOn (ToggleSchedule True)) ]
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.SecondaryIcon Icon.CalendarOff (ToggleSchedule False)) ]
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.SecondaryIcon Icon.Refresh (QuickAction RefreshRender)) ]
                    , Html.div [ ATT.class "ml-2" ]
                        [ Button.view (Button.SecondaryIcon Icon.ClearCircle (QuickAction ClearRender)) ]
                    ]
    in
    case model.loadedDevice of
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
                , Html.div [ ATT.class "sm:flex sm:items-start mt-4 pt-4 border-t border-solid border-neutral-500" ]
                    [ Html.div [ ATT.class "flex-1 pb-4 sm:pb-0 sm:mr-2 sm:pr-2" ]
                        [ deviceInfoTable model info ]
                    , Html.div [ ATT.class "flex-1 border-t pt-4 sm:ml-2 sm:pl-2 sm:border-t-0 sm:pt-0 border-neutral-500" ]
                        [ deviceAuhtorityInfo model ]
                    ]
                ]


deviceAuhtorityInfo : Model -> Html.Html Message
deviceAuhtorityInfo model =
    case model.loadedAuthority of
        Just (Ok auth) ->
            Html.div [ ATT.class "flex items-center" ]
                [ Html.div [ ATT.class "mr-2" ] [ Icon.view (DA.icon auth.authorityModel) ]
                , Html.div [] [ Html.text auth.authorityModel.kind ]
                ]

        Just (Err _) ->
            Html.div [] [ Html.text "unable to load authority model" ]

        Nothing ->
            Html.div [] [ Html.text "loading authority model..." ]


modelInfoHeader : Model -> Html.Html Message
modelInfoHeader model =
    case model.loadedDevice of
        Just (Ok info) ->
            case info.nickname of
                Just name ->
                    Html.div [ ATT.class "flex items-center" ]
                        [ Html.div [] [ Html.text name ]
                        , Html.div [ ATT.class "ml-2" ] [ Html.code [] [ Html.text info.id ] ]
                        ]

                Nothing ->
                    Html.div [] [ Html.text model.id ]

        Just (Err e) ->
            Html.div [] [ Html.text model.id ]

        Nothing ->
            Html.div [] [ Html.text model.id ]


deviceInfoTable : Model -> DeviceInfoResponse -> Html.Html Message
deviceInfoTable model info =
    let
        sentMessageCount =
            Maybe.withDefault 0 info.sentMessageCount |> String.fromInt

        ( lastSeenText, firstSeenText ) =
            case model.currentTime of
                Just time ->
                    let
                        lastDiff =
                            TimeDiff.diff time (Time.millisToPosix info.lastSeen)

                        firstDiff =
                            TimeDiff.diff time (Time.millisToPosix info.firstSeen)
                    in
                    ( Html.text (TimeDiff.toString lastDiff), Html.text (TimeDiff.toString firstDiff) )

                Nothing ->
                    ( Html.text (TimeDiff.formatDeviceTime info.lastSeen ++ "UTC")
                    , Html.text (TimeDiff.formatDeviceTime info.firstSeen ++ "UTC")
                    )
    in
    Html.table [ ATT.class "w-full mt-2" ]
        [ Html.thead [] []
        , Html.tbody []
            [ Html.tr []
                [ Html.td [ ATT.class "whitespace-nowrap text-ellipsis" ] [ Html.text "Total Messages Sent" ]
                , Html.td [] [ Html.text sentMessageCount ]
                ]
            , Html.tr []
                [ Html.td [ ATT.class "whitespace-nowrap text-ellipsis" ] [ Html.text "Current Queue" ]
                , Html.td [] [ Html.text (String.fromInt info.currentQueueCount) ]
                ]
            , Html.tr []
                [ Html.td [ ATT.class "whitespace-nowrap text-ellipsis" ] [ Html.text "Last Seen" ]
                , Html.td [ ATT.title (TimeDiff.formatDeviceTime info.lastSeen) ] [ lastSeenText ]
                ]
            , Html.tr []
                [ Html.td [ ATT.class "whitespace-nowrap text-ellipsis" ] [ Html.text "First Seen" ]
                , Html.td [] [ firstSeenText ]
                ]
            ]
        ]


infoDecoder : D.Decoder DeviceInfoResponse
infoDecoder =
    D.map6 DeviceInfoResponse
        (D.field "id" D.string)
        (D.field "last_seen" D.int)
        (D.field "nickname" (D.maybe D.string))
        (D.field "first_seen" D.int)
        (D.field "sent_message_count" (D.maybe D.int))
        (D.field "current_queue_count" D.int)


fetchDevice : Environment.Environment -> String -> Cmd Message
fetchDevice env id =
    Http.get
        { url = Environment.apiRoute env ("device-info?id=" ++ id)
        , expect = Http.expectJson LoadedDeviceInfo infoDecoder
        }


setActiveInputText : String -> InputKinds -> InputKinds
setActiveInputText newValue kind =
    case kind of
        DeviceName _ ->
            DeviceName newValue

        Message _ ->
            Message newValue

        Link _ ->
            Link newValue


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        ClearAlert ->
            ( { model | alert = Nothing }, Cmd.none )

        SettingsMenuUpdate dropdown (Just StartRename) ->
            ( { model | settingsMenu = dropdown, activeInput = ( DeviceName "", Nothing ) }, Cmd.none )

        SettingsMenuUpdate dropdown (Just QueueWelcomeScannable) ->
            ( { model | settingsMenu = dropdown }, DQ.postMessage env QueuedMessageJob model.id DQ.WelcomeMessage )

        SettingsMenuUpdate dropdown (Just MakePublic) ->
            ( { model | settingsMenu = dropdown }, DQ.postMessage env QueuedMessageJob model.id (DQ.PublicAccessChange True) )

        SettingsMenuUpdate dropdown (Just MakePrivate) ->
            ( { model | settingsMenu = dropdown }, DQ.postMessage env QueuedMessageJob model.id (DQ.PublicAccessChange False) )

        SettingsMenuUpdate dropdown Nothing ->
            ( { model | settingsMenu = dropdown }, Cmd.none )

        Tick time ->
            let
                ( refreshCommand, pendingRefresh ) =
                    case model.pendingRefresh of
                        Just Nothing ->
                            ( Cmd.none, model.pendingRefresh )

                        Nothing ->
                            ( fetchDevice env model.id, Just Nothing )

                        Just (Just _) ->
                            ( fetchDevice env model.id, Just Nothing )

                pollCommand =
                    case List.head model.pendingMessageJobs of
                        Just handle ->
                            Job.loadPendingJob env (LoadedJobHandle handle) handle

                        Nothing ->
                            Cmd.none
            in
            ( { model | currentTime = Just time, pendingRefresh = pendingRefresh }
            , Cmd.batch [ refreshCommand, pollCommand ]
            )

        LoadedDeviceAuthority authorityResult ->
            ( { model | loadedAuthority = Just authorityResult }, Cmd.none )

        LoadedJobHandle handle (Ok job) ->
            let
                jobResult =
                    Job.asResult job
            in
            case jobResult of
                Job.Pending ->
                    ( model, Cmd.none )

                -- TODO(job-polling): this clears out the job being polled whenever it reaches a terminal
                --                    state. eventually we will want to handle failures much better.
                _ ->
                    ( { model | pendingMessageJobs = [] }, DA.fetchDeviceAuthority env model.id LoadedDeviceAuthority )

        LoadedJobHandle handle (Err _) ->
            ( { model | pendingMessageJobs = [] }, Cmd.none )

        UpdateInput newInput ->
            ( { model | activeInput = ( newInput, Tuple.second model.activeInput ) }, Cmd.none )

        SetMessage messageText ->
            let
                nextInput =
                    Tuple.first model.activeInput
                        |> setActiveInputText messageText
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
                    Tuple.first model.activeInput |> resetInput
            in
            ( { model | activeInput = ( emptiedInput, Nothing ) }, Cmd.none )

        ToggleSchedule state ->
            ( { model | activeInput = ( Tuple.first model.activeInput, Just Nothing ) }
            , DQ.postMessage env QueuedMessageJob model.id (DQ.SchedulePayload state)
            )

        QuickAction q ->
            let
                cmd =
                    case q of
                        RefreshRender ->
                            DQ.postMessage env QueuedMessageJob model.id DQ.Refresh

                        ClearRender ->
                            DQ.postMessage env QueuedMessageJob model.id DQ.Clear
            in
            ( model, cmd )

        ToggleLights state ->
            ( { model | activeInput = ( Tuple.first model.activeInput, Just Nothing ) }
            , DQ.postMessage env QueuedMessageJob model.id (DQ.LightPayload state)
            )

        --
        QueuedMessageJob (Ok jobHandle) ->
            let
                jobList =
                    jobHandle :: model.pendingMessageJobs

                activeInput =
                    Tuple.first model.activeInput |> resetInput
            in
            ( { model | pendingMessageJobs = jobList, activeInput = ( activeInput, Nothing ) }, Cmd.none )

        QueuedMessageJob (Err e) ->
            let
                activeInput =
                    Tuple.first model.activeInput |> resetInput

                alert =
                    Just (Alert.Warning "Unable to queue")
            in
            ( { model | alert = alert, activeInput = ( activeInput, Nothing ) }, Cmd.none )

        AttemptMessage ->
            let
                payload =
                    case Tuple.first model.activeInput of
                        DeviceName str ->
                            DQ.DeviceRenamePayload str

                        Message str ->
                            DQ.MessagePayload str

                        Link str ->
                            DQ.LinkPayload str

                newInput =
                    ( Tuple.first model.activeInput, Just Nothing )
            in
            ( { model | activeInput = newInput }, DQ.postMessage env QueuedMessageJob model.id payload )

        LoadedTime now ->
            ( { model | currentTime = Just now }, Cmd.none )


getNow : Cmd Message
getNow =
    Task.perform LoadedTime Time.now


viewAlert : Model -> Html.Html Message
viewAlert model =
    case model.alert of
        Just a ->
            Html.div [ ATT.class "mb-4 w-full" ]
                [ Alert.view a ClearAlert ]

        Nothing ->
            Html.div [] []


default : Environment.Environment -> String -> ( Model, Cmd Message )
default env id =
    ( { id = id
      , activeInput = ( Message "", Nothing )
      , loadedDevice = Nothing
      , loadedAuthority = Nothing
      , pendingMessageJobs = []
      , pendingRefresh = Nothing
      , currentTime = Nothing
      , settingsMenu = Dropdown.empty
      , alert = Nothing
      }
    , Cmd.batch
        [ fetchDevice env id
        , getNow
        , DA.fetchDeviceAuthority env
            id
            LoadedDeviceAuthority
        , DS.fetchDeviceSchedule
            env
            id
            Loaded
        ]
    )
