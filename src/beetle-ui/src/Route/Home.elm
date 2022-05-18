module Route.Home exposing (Message, Model, default, update, view)

import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Random


type alias Data =
    { devices : List String
    , newDevice : ( String, Maybe (Maybe Http.Error) )
    }


type alias Model =
    Maybe (Result Http.Error Data)


type Message
    = SetNewDeviceId String
    | AttemptDeviceClaim
    | Pretend Int


emptyData : Data
emptyData =
    { devices = [], newDevice = ( "", Nothing ) }


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


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        Pretend item ->
            ( Just (Ok emptyData), Cmd.none )

        AttemptDeviceClaim ->
            case model of
                Nothing ->
                    ( Nothing, Cmd.none )

                Just res ->
                    case res of
                        Ok data ->
                            let
                                ( id, _ ) =
                                    data.newDevice
                            in
                            ( Just (Ok { data | newDevice = ( id, Just Nothing ) }), Cmd.none )

                        Err e ->
                            ( Just (Err e), Cmd.none )

        SetNewDeviceId id ->
            case model of
                Nothing ->
                    ( Nothing, Cmd.none )

                Just res ->
                    case res of
                        Ok data ->
                            ( Just (Ok { data | newDevice = ( id, Nothing ) }), Cmd.none )

                        Err e ->
                            ( Just (Err e), Cmd.none )


view : Model -> Html.Html Message
view model =
    case model of
        Nothing ->
            Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "loading..." ]

        Just result ->
            case result of
                Err error ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ] [ Html.text "failed, please refresh" ]

                Ok data ->
                    Html.div [ Html.Attributes.class "flex px-4 py-3" ]
                        [ Html.div [ Html.Attributes.class "flex-1" ] [ Html.text "devices" ]
                        , Html.div [ Html.Attributes.class "flex-1" ]
                            [ Html.div [ Html.Attributes.class "px-3 py-2" ] [ Html.text "add-device" ]
                            , Html.div [ Html.Attributes.class "flex items-center" ]
                                [ Html.input
                                    [ Html.Attributes.placeholder "device id"
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
                            ]
                        ]


oneToTen : Random.Generator Int
oneToTen =
    Random.int 1 10


default : Environment.Environment -> ( Model, Cmd Message )
default env =
    ( Nothing, Random.generate Pretend oneToTen )
