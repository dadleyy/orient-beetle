module Route.Home exposing (Message, Model, default, update, view)

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


type alias Data =
    { devices : List String
    , newDevice : ( String, Maybe (Maybe Http.Error) )
    , alert : Maybe Alert
    }


type alias Model =
    Maybe (Result Http.Error Data)


type Message
    = SetNewDeviceId String
    | AttemptDeviceClaim
    | RegisteredDevice (Result Http.Error RegistrationResponse)
    | Pretend Int


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


registerDevice : Environment.Environment -> String -> Cmd Message
registerDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


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


sendAttempt : Environment.Environment -> String -> Cmd Message
sendAttempt env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


setNewDeviceId : String -> Data -> Data
setNewDeviceId id model =
    { model | newDevice = ( id, Nothing ), alert = Nothing }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        Pretend item ->
            ( Just (Ok emptyData), Cmd.none )

        RegisteredDevice registrationResult ->
            ( model |> Maybe.map (Result.map (applyRegistrationResult registrationResult)), Cmd.none )

        AttemptDeviceClaim ->
            let
                cmd =
                    Maybe.withDefault Cmd.none (Maybe.map (sendAttempt env) (getDeviceId model))
            in
            ( model |> Maybe.map (Result.map setPending), cmd )

        SetNewDeviceId id ->
            ( model |> Maybe.map (Result.map (setNewDeviceId id)), Cmd.none )


deviceRegistrationForm : Data -> Html.Html Message
deviceRegistrationForm data =
    let
        ( value, _ ) =
            data.newDevice
    in
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.div [ Html.Attributes.class "px-3 py-2" ] [ Html.text "add-device" ]
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
                [ Html.text "add" ]
            ]
        , case data.alert of
            Nothing ->
                Html.div [] []

            Just (Happy text) ->
                Html.div [ Html.Attributes.class "mt-2 pill happy" ] [ Html.text text ]

            Just (Warning text) ->
                Html.div [ Html.Attributes.class "mt-2 pill sad" ] [ Html.text text ]
        ]


deviceList : Data -> Html.Html Message
deviceList data =
    Html.div [ Html.Attributes.class "flex-1" ] []


view : Model -> Html.Html Message
view model =
    case model of
        Nothing ->
            Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "loading..." ]

        Just result ->
            case result of
                Err error ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "failed, please refresh" ]

                Ok modelData ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ]
                        [ deviceList modelData, deviceRegistrationForm modelData ]


oneToTen : Random.Generator Int
oneToTen =
    Random.int 1 10


default : Environment.Environment -> ( Model, Cmd Message )
default env =
    ( Nothing, Random.generate Pretend oneToTen )
