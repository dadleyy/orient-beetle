module Route.Device exposing (Message(..), Model, default, update, view)

import Environment
import Html
import Html.Attributes
import Html.Events
import Http
import Json.Encode


type Message
    = Loaded (Result Http.Error ())
    | AttemptMessage
    | SetMessage String


type alias Model =
    { id : String
    , newMessage : ( String, Maybe (Maybe String) )
    }


setMessage : Model -> String -> Model
setMessage model message =
    { model | newMessage = ( message, Nothing ) }


getMessage : Model -> String
getMessage model =
    Tuple.first model.newMessage


isBusy : Model -> Bool
isBusy model =
    Tuple.second model.newMessage |> Maybe.map (always True) |> Maybe.withDefault False


view : Model -> Environment.Environment -> Html.Html Message
view model env =
    Html.div
        [ Html.Attributes.class "px-4 py-3"
        ]
        [ Html.div [ Html.Attributes.class "pb-1 mb-1" ] [ Html.h2 [] [ Html.text model.id ] ]
        , Html.div [ Html.Attributes.class "flex items-center" ]
            [ Html.input
                [ Html.Events.onInput SetMessage
                , Html.Attributes.value (getMessage model)
                , Html.Attributes.disabled (isBusy model)
                ]
                []
            , Html.button
                [ Html.Events.onClick AttemptMessage, Html.Attributes.disabled (isBusy model) ]
                [ Html.text "send" ]
            ]
        ]


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


fetchDevice : Environment.Environment -> String -> Cmd Message
fetchDevice env id =
    Http.get
        { url = Environment.apiRoute env ("device-info?id=" ++ id)
        , expect = Http.expectWhatever Loaded
        }


update : Environment.Environment -> Message -> Model -> ( Model, Cmd Message )
update env message model =
    case message of
        SetMessage messageText ->
            ( setMessage model messageText, Cmd.none )

        Loaded _ ->
            ( { model | newMessage = ( "", Nothing ) }, Cmd.none )

        AttemptMessage ->
            ( { model | newMessage = ( Tuple.first model.newMessage, Just Nothing ) }, postMessage env model )


default : Environment.Environment -> String -> ( Model, Cmd Message )
default env id =
    ( { id = id, newMessage = ( "", Nothing ) }, Cmd.batch [ fetchDevice env id ] )
