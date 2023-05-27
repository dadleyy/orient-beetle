module Route.Home exposing (Message, Model, default, update, view)

import Button
import Dict
import Environment
import Html
import Html.Attributes as A
import Html.Events
import Http
import Icon
import Json.Decode as D
import Json.Encode
import Random


type Alert
    = Warning String
    | Happy String


type alias DeviceSnapshot =
    { nickname : Maybe String }


type alias OwnedDevice =
    { id : String
    , nickname : Maybe String
    , busy : Bool
    }


type alias Data =
    { devices : Result Http.Error (List OwnedDevice)
    , newDevice : ( String, Maybe (Maybe Http.Error) )
    , alert : Maybe Alert
    }


type alias Model =
    Maybe (Result Http.Error Data)


type alias DeviceList =
    { devices : Dict.Dict String DeviceSnapshot }


type Message
    = SetNewDeviceId String
    | AttemptDeviceClaim
    | RegisteredDevice (Result Http.Error RegistrationResponse)
    | AttemptDeviceRemove String
    | RemovedDevice (Result Http.Error ())
    | LoadedDevices (Result Http.Error DeviceList)


type alias RegistrationResponse =
    { id : String }


emptyData : Data
emptyData =
    { devices = Ok [], newDevice = ( "", Nothing ), alert = Nothing }


hasPendingAddition : Data -> Bool
hasPendingAddition data =
    let
        ( _, attempt ) =
            data.newDevice
    in
    case attempt of
        Just _ ->
            True

        Nothing ->
            False


registrationDecoder : D.Decoder RegistrationResponse
registrationDecoder =
    D.map RegistrationResponse (D.field "id" D.string)


removeDevice : Environment.Environment -> String -> Cmd Message
removeDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/unregister"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectWhatever RemovedDevice
        }


registerDevice : Environment.Environment -> String -> Cmd Message
registerDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


setAlert : String -> Data -> Data
setAlert message data =
    { data | alert = Just (Warning message) }


applyRegistrationResult : Result Http.Error RegistrationResponse -> Data -> Data
applyRegistrationResult result data =
    case result of
        Err (Http.BadUrl url) ->
            { data | alert = Just (Warning (String.concat [ "bad-url: ", url ])) }

        Err Http.Timeout ->
            { data | alert = Just (Warning "timeout") }

        Err Http.NetworkError ->
            { data | alert = Just (Warning "unable to connect") }

        Err (Http.BadStatus _) ->
            { data | alert = Just (Warning "bad request (response)") }

        Err (Http.BadBody _) ->
            { data | alert = Just (Warning "bad request (body)") }

        Ok res ->
            { data | alert = Just (Happy "ok"), newDevice = ( "", Nothing ) }


setPending : Data -> Data
setPending data =
    let
        ( id, _ ) =
            data.newDevice
    in
    { data | newDevice = ( id, Just Nothing ) }


getDeviceId : Model -> Maybe String
getDeviceId model =
    case Maybe.andThen Result.toMaybe model of
        Just data ->
            let
                ( id, _ ) =
                    data.newDevice
            in
            Just id

        Nothing ->
            Nothing


addDevice : Environment.Environment -> String -> Cmd Message
addDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


setNewDeviceId : String -> Data -> Data
setNewDeviceId id model =
    { model | newDevice = ( id, Nothing ), alert = Nothing }


ownedDevice : String -> OwnedDevice
ownedDevice id =
    { id = id, nickname = Nothing, busy = False }


checkBusy : String -> OwnedDevice -> OwnedDevice
checkBusy id dev =
    { dev | busy = dev.busy || id == dev.id }


markDeviceBusy : String -> Data -> Data
markDeviceBusy id data =
    { data | devices = Result.map (List.map (checkBusy id)) data.devices }


ownedDeviceFromKeyValue : ( String, DeviceSnapshot ) -> OwnedDevice
ownedDeviceFromKeyValue pair =
    let
        ( id, snapshot ) =
            pair
    in
    { id = id, busy = False, nickname = snapshot.nickname }


deviceListFromMap : Dict.Dict String DeviceSnapshot -> List OwnedDevice
deviceListFromMap mapping =
    Dict.toList mapping |> List.map ownedDeviceFromKeyValue


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        LoadedDevices deviceResponse ->
            case ( deviceResponse, model ) of
                ( Err e, maybeModel ) ->
                    let
                        newModel =
                            case maybeModel of
                                Just (Ok m) ->
                                    { m | devices = Err e }

                                _ ->
                                    { emptyData | devices = Err e }
                    in
                    ( Just (Ok newModel), Cmd.none )

                ( Ok responseData, Just (Ok loadedModel) ) ->
                    let
                        devices =
                            deviceListFromMap responseData.devices
                    in
                    ( Just (Ok { loadedModel | devices = Ok devices }), Cmd.none )

                ( Ok responseData, _ ) ->
                    let
                        devices =
                            deviceListFromMap responseData.devices
                    in
                    ( Just (Ok { emptyData | devices = Ok devices }), Cmd.none )

        AttemptDeviceClaim ->
            let
                cmd =
                    Maybe.withDefault Cmd.none (Maybe.map (addDevice env) (getDeviceId model))
            in
            ( model |> Maybe.map (Result.map setPending), cmd )

        RegisteredDevice registrationResult ->
            ( model |> Maybe.map (Result.map (applyRegistrationResult registrationResult)), fetchDevices env )

        AttemptDeviceRemove id ->
            ( model |> Maybe.map (Result.map (markDeviceBusy id)), removeDevice env id )

        RemovedDevice result ->
            case result of
                Err _ ->
                    ( model |> Maybe.map (Result.map (setAlert "Unable to remove")), fetchDevices env )

                Ok _ ->
                    ( model, fetchDevices env )

        SetNewDeviceId id ->
            ( model |> Maybe.map (Result.map (setNewDeviceId id)), Cmd.none )


renderDevice : Environment.Environment -> OwnedDevice -> Html.Html Message
renderDevice env device =
    let
        linkContents =
            case device.nickname of
                Just name ->
                    Html.div [ A.title device.id ] [ Html.text name ]

                Nothing ->
                    Html.div [] [ Html.text device.id ]

        linkUrl =
            Environment.buildRoutePath env ("devices/" ++ device.id)
    in
    Html.tr []
        [ Html.td
            [ A.class "px-3 py-2" ]
            [ Html.a
                [ A.href linkUrl ]
                [ linkContents ]
            ]
        , Html.td [ A.class "px-3 py-2 text-right" ]
            (if device.busy then
                [ Button.view (Button.DisabledIcon Icon.Trash) ]

             else
                [ Button.view (Button.PrimaryIcon Icon.Trash (AttemptDeviceRemove device.id)) ]
            )
        ]


deviceList : Data -> Environment.Environment -> Html.Html Message
deviceList data env =
    let
        addButton =
            Button.view (Button.LinkIcon Icon.Add (Environment.buildRoutePath env "register-device"))

        body =
            case data.devices of
                Ok list ->
                    Html.tbody [] (List.map (renderDevice env) list)

                Err error ->
                    Html.tbody [] [ Html.tr [] [ Html.td [] [ Html.text "Failed to load" ] ] ]
    in
    Html.div [ A.class "flex-1" ]
        [ Html.table [ A.class "w-full" ]
            [ Html.thead []
                [ Html.tr [ A.class "text-left" ]
                    [ Html.th [ A.class "px-3 pb-2" ] [ Html.text "Devices" ]
                    , Html.th [ A.class "px-3 py-2 text-right" ]
                        [ addButton ]
                    ]
                ]
            , body
            ]
        ]


view : Model -> Environment.Environment -> Html.Html Message
view model env =
    case model of
        Nothing ->
            Html.div [ A.class "flex px-4 py-3" ] [ Html.text "Loading..." ]

        Just result ->
            case result of
                Err error ->
                    Html.div [ A.class "flex px-4 py-3" ]
                        [ Html.text "failed, please refresh" ]

                Ok modelData ->
                    Html.div [ A.class "flex px-4 py-3" ]
                        [ deviceList modelData env ]


sessionDecoder : D.Decoder DeviceList
sessionDecoder =
    D.map DeviceList (D.field "devices" (D.dict snapshotDecoder))



-- TODO(API): the api returns devices in the same payload as other user information. That will probably
-- not paginate well.


snapshotDecoder : D.Decoder DeviceSnapshot
snapshotDecoder =
    D.map DeviceSnapshot (D.field "nickname" (D.maybe D.string))


fetchDevices : Environment.Environment -> Cmd Message
fetchDevices env =
    Http.get
        { url = Environment.apiRoute env "auth/identify"
        , expect = Http.expectJson LoadedDevices sessionDecoder
        }


default : Environment.Environment -> ( Model, Cmd Message )
default env =
    ( Nothing, Cmd.batch [ fetchDevices env ] )
