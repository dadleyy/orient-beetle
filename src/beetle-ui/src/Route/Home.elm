module Route.Home exposing (Message, Model, default, update, view)

import Dict
import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Json.Decode
import Json.Encode
import Random


type Alert
    = Warning String
    | Happy String


type alias OwnedDevice =
    { id : String
    , busy : Bool
    }


type alias Data =
    { devices : List OwnedDevice
    , newDevice : ( String, Maybe (Maybe Http.Error) )
    , alert : Maybe Alert
    }


type alias Model =
    Maybe (Result Http.Error Data)


type alias DeviceList =
    { devices : Dict.Dict String Int
    }


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
    { devices = [], newDevice = ( "", Nothing ), alert = Nothing }


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


registrationDecoder : Json.Decode.Decoder RegistrationResponse
registrationDecoder =
    Json.Decode.map RegistrationResponse (Json.Decode.field "id" Json.Decode.string)


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
    OwnedDevice id False


checkBusy : String -> OwnedDevice -> OwnedDevice
checkBusy id dev =
    { dev | busy = dev.busy || id == dev.id }


markDeviceBusy : String -> Data -> Data
markDeviceBusy id data =
    { data | devices = List.map (checkBusy id) data.devices }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        LoadedDevices item ->
            case item of
                Ok inner ->
                    case model of
                        Just (Ok data) ->
                            ( Just (Ok { data | devices = Dict.keys inner.devices |> List.map ownedDevice }), Cmd.none )

                        _ ->
                            ( Just (Ok { emptyData | devices = Dict.keys inner.devices |> List.map ownedDevice }), Cmd.none )

                Err e ->
                    ( Just (Ok emptyData), Cmd.none )

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


deviceRegistrationForm : Data -> Html.Html Message
deviceRegistrationForm data =
    let
        ( value, _ ) =
            data.newDevice
    in
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.div [ Html.Attributes.class "px-3 py-2" ] [ Html.b [] [ Html.text "Add Device" ] ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input
                [ Html.Attributes.placeholder "device id"
                , Html.Attributes.value value
                , Html.Attributes.class "block mr-2"
                , Html.Attributes.disabled (hasPendingAddition data)
                , Html.Events.onInput SetNewDeviceId
                ]
                []
            , Html.button
                [ Html.Attributes.disabled (hasPendingAddition data)
                , Html.Events.onClick AttemptDeviceClaim
                ]
                [ Html.text "Add" ]
            ]
        , case data.alert of
            Nothing ->
                Html.div [] []

            Just (Happy text) ->
                Html.div [ Html.Attributes.class "mt-2 pill happy" ] [ Html.text text ]

            Just (Warning text) ->
                Html.div [ Html.Attributes.class "mt-2 pill sad" ] [ Html.text text ]
        ]


renderDevice : OwnedDevice -> Html.Html Message
renderDevice device =
    Html.tr []
        [ Html.td [ Html.Attributes.class "px-3 py-2" ] [ Html.text device.id ]
        , Html.td
            [ Html.Attributes.class "px-3 py-2" ]
            [ Html.button
                [ Html.Attributes.disabled device.busy
                , Html.Events.onClick (AttemptDeviceRemove device.id)
                ]
                [ Html.text "Remove" ]
            ]
        ]


deviceList : Data -> Html.Html Message
deviceList data =
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.table [ Html.Attributes.class "w-full" ]
            [ Html.thead []
                [ Html.tr [ Html.Attributes.class "text-left" ]
                    [ Html.th [ Html.Attributes.class "px-3 py-2" ] [ Html.text "Devices" ]
                    , Html.th [ Html.Attributes.class "px-3 py-2" ] []
                    ]
                ]
            , Html.tbody [] (List.map renderDevice data.devices)
            ]
        ]


view : Model -> Html.Html Message
view model =
    case model of
        Nothing ->
            Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "Loading..." ]

        Just result ->
            case result of
                Err error ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "failed, please refresh" ]

                Ok modelData ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ]
                        [ deviceList modelData, deviceRegistrationForm modelData ]


sessionDecoder : Json.Decode.Decoder DeviceList
sessionDecoder =
    Json.Decode.map DeviceList (Json.Decode.field "devices" (Json.Decode.dict Json.Decode.int))


fetchDevices : Environment.Environment -> Cmd Message
fetchDevices env =
    Http.get { url = Environment.apiRoute env "auth/identify", expect = Http.expectJson LoadedDevices sessionDecoder }


default : Environment.Environment -> ( Model, Cmd Message )
default env =
    ( Nothing, Cmd.batch [ fetchDevices env ] )
