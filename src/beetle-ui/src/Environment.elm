module Environment exposing
    ( Configuration
    , Environment
    , Message(..)
    , Session
    , StatusResponse
    , apiRoute
    , boot
    , buildRoutePath
    , default
    , getId
    , getLoadedId
    , getLocalizedContent
    , getSession
    , isLoaded
    , normalizeUrlPath
    , statusFooter
    , update
    )

import Browser.Navigation as Nav
import Dict
import Html
import Http
import Json.Decode
import Url


type alias StatusResponse =
    { version : String
    , timestamp : String
    }


type alias Session =
    { oid : String
    , picture : String
    }


type alias Configuration =
    { api : String
    , apiDocsUrl : String
    , root : String
    , loginUrl : String
    , logoutUrl : String
    , localization : List ( String, String )
    }


type alias Environment =
    { configuration : Configuration
    , status : Maybe (Result String StatusResponse)
    , session : Maybe (Result String Session)
    , navKey : Nav.Key
    }


type Message
    = LoadedStatus (Result Http.Error StatusResponse)
    | LoadedSession (Result Http.Error Session)


default : Configuration -> Nav.Key -> Environment
default configuration key =
    { configuration = configuration
    , status = Nothing
    , session = Nothing
    , navKey = key
    }


errorForHttp : Http.Error -> String
errorForHttp error =
    case error of
        Http.BadStatus _ ->
            "Unable to fetch from beetle server"

        _ ->
            "Unknown problem"


isLoaded : Environment -> Bool
isLoaded env =
    let
        hasSession =
            Maybe.map (always True) env.session |> Maybe.withDefault False

        hasStatus =
            Maybe.map (always True) env.status |> Maybe.withDefault False
    in
    hasStatus && hasSession



-- TODO: Currently this is responsible for handling messages that are returned from the
-- environment's session-related XHR commands. In addition, it returns a (Maybe String)
-- which indicates the url that we "should" send the user to based on that information.


update : Message -> Environment -> Environment
update message environment =
    case message of
        LoadedStatus result ->
            { environment | status = Just (Result.mapError errorForHttp result) }

        LoadedSession result ->
            { environment | session = Just (Result.mapError errorForHttp result) }


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


getSession : Environment -> Maybe Session
getSession env =
    Maybe.andThen Result.toMaybe env.session


getId : Environment -> Maybe String
getId env =
    getSession env |> Maybe.map getSessionId


getLoadedId : Environment -> Maybe (Maybe String)
getLoadedId env =
    case env.session |> Maybe.map Result.toMaybe of
        Just (Just session) ->
            Just (Just session.oid)

        Just Nothing ->
            Just Nothing

        Nothing ->
            Nothing


sessionDecoder : Json.Decode.Decoder Session
sessionDecoder =
    Json.Decode.map2 Session
        (Json.Decode.field "oid" Json.Decode.string)
        (Json.Decode.field "picture" Json.Decode.string)


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


normalizeUrlPath : Environment -> Url.Url -> String
normalizeUrlPath env url =
    String.dropLeft (String.length env.configuration.root) url.path


buildRoutePath : Environment -> String -> String
buildRoutePath env path =
    String.concat
        [ env.configuration.root, path ]


getLocalizedContent : Environment -> String -> Maybe String
getLocalizedContent env key =
    let
        matchingKeys =
            List.filter (\item -> Tuple.first item == key) env.configuration.localization
    in
    List.head matchingKeys |> Maybe.map Tuple.second
