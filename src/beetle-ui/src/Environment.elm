module Environment exposing (Configuration, Environment, Message(..), Session, StatusResponse, boot, buildRoutePath, default, getId, statusFooter, update)

import Html
import Http
import Json.Decode


type alias StatusResponse =
    { version : String
    , timestamp : String
    }


type alias Session =
    { oid : String
    }


type alias Configuration =
    { api : String
    , root : String
    , loginUrl : String
    }


type alias Environment =
    { configuration : Configuration
    , status : Maybe (Result String StatusResponse)
    , session : Maybe (Result String Session)
    }


type Message
    = LoadedStatus (Result Http.Error StatusResponse)
    | LoadedSession (Result Http.Error Session)


default : Configuration -> Environment
default configuration =
    { configuration = configuration
    , status = Nothing
    , session = Nothing
    }


errorForHttp : Http.Error -> String
errorForHttp error =
    case error of
        Http.BadStatus _ ->
            "Unable to fetch from beetle server"

        _ ->
            "Unknown problem"


update : Message -> Environment -> ( Environment, Maybe String )
update message environment =
    case message of
        LoadedStatus result ->
            ( { environment
                | status = Just (Result.mapError errorForHttp result)
              }
            , Nothing
            )

        LoadedSession result ->
            let
                destination =
                    case result of
                        Err _ ->
                            "/login"

                        Ok _ ->
                            "/home"
            in
            ( { environment | session = Just (Result.mapError errorForHttp result) }, Just destination )


apiRoute : Environment -> String -> String
apiRoute env path =
    String.concat [ env.configuration.api, path ]


boot : Environment -> Cmd Message
boot env =
    Cmd.batch
        [ Http.get { url = apiRoute env "status", expect = Http.expectJson LoadedStatus statusDecoder }
        , Http.get { url = apiRoute env "auth/identify", expect = Http.expectJson LoadedSession sessionDecoder }
        ]


getSessionId : Session -> String
getSessionId session =
    session.oid


getId : Environment -> Maybe String
getId env =
    Maybe.map getSessionId (Maybe.andThen Result.toMaybe env.session)


sessionDecoder : Json.Decode.Decoder Session
sessionDecoder =
    Json.Decode.map Session
        (Json.Decode.field "oid" Json.Decode.string)


statusDecoder : Json.Decode.Decoder StatusResponse
statusDecoder =
    Json.Decode.map2 StatusResponse
        (Json.Decode.field "version" Json.Decode.string)
        (Json.Decode.field "timestamp" Json.Decode.string)


statusFooter : Environment -> Html.Html Message
statusFooter env =
    case env.status of
        Just result ->
            case result of
                Err error ->
                    Html.div [] [ Html.text (String.concat [ "failed: ", error ]) ]

                Ok response ->
                    Html.div [] [ Html.text (String.concat [ String.slice 0 7 response.version, " @ ", response.timestamp ]) ]

        Nothing ->
            Html.div [] [ Html.text "Connecting..." ]


buildRoutePath : Environment -> String -> String
buildRoutePath env path =
    String.concat [ env.configuration.root, path ]
