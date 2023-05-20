module Route.DeviceRegistration exposing (Message(..), Model, default, update, view, withInitialId)

import Browser.Navigation as Nav
import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Json.Decode
import Json.Encode


type alias RegistrationResponse =
    { id : String }


type alias Model =
    { newDevice : ( String, Maybe (Maybe Http.Error) )
    , alert : Maybe Alert
    }


type Message
    = SetNewDeviceId String
    | AttemptDeviceClaim
    | RegisteredDevice (Result Http.Error RegistrationResponse)


type Alert
    = Warning String
    | Happy String


default : Model
default =
    { newDevice = ( "", Nothing ), alert = Nothing }


withInitialId : String -> Model
withInitialId id =
    { newDevice = ( id, Nothing ), alert = Nothing }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        SetNewDeviceId id ->
            ( { model | newDevice = ( id, Nothing ) }, Cmd.none )

        AttemptDeviceClaim ->
            ( { model | newDevice = ( Tuple.first model.newDevice, Just Nothing ) }
            , addDevice env (Tuple.first model.newDevice)
            )

        RegisteredDevice result ->
            case result of
                Ok registrationRes ->
                    ( model, Nav.pushUrl env.navKey ("/devices/" ++ registrationRes.id) )

                Err error ->
                    ( { model | newDevice = ( "", Nothing ), alert = Just (Warning "Failed") }, Cmd.none )


view : Environment.Environment -> Model -> Html.Html Message
view env model =
    Html.div [ Html.Attributes.class "px-4 py-3" ] [ deviceRegistrationForm env model ]


hasPendingAddition : Model -> Bool
hasPendingAddition model =
    Tuple.second model.newDevice |> Maybe.map (always True) |> Maybe.withDefault False


addDevice : Environment.Environment -> String -> Cmd Message
addDevice env id =
    Http.post
        { url = Environment.apiRoute env "devices/register"
        , body = Http.jsonBody (Json.Encode.object [ ( "device_id", Json.Encode.string id ) ])
        , expect = Http.expectJson RegisteredDevice registrationDecoder
        }


registrationDecoder : Json.Decode.Decoder RegistrationResponse
registrationDecoder =
    Json.Decode.map RegistrationResponse (Json.Decode.field "id" Json.Decode.string)


deviceRegistrationForm : Environment.Environment -> Model -> Html.Html Message
deviceRegistrationForm env model =
    Html.div [ Html.Attributes.class "flex-1" ]
        [ Html.div [ Html.Attributes.class "pb-3 py-2" ] [ Html.b [] [ Html.text "Add Device" ] ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input
                [ Html.Attributes.placeholder "device id"
                , Html.Attributes.value (Tuple.first model.newDevice)
                , Html.Attributes.class "block mr-2"
                , Html.Attributes.disabled (hasPendingAddition model)
                , Html.Events.onInput SetNewDeviceId
                ]
                []
            , Html.button
                [ Html.Attributes.disabled (hasPendingAddition model || String.isEmpty (Tuple.first model.newDevice))
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
